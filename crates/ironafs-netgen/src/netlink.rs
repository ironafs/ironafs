// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The route-netlink messages the monitor reads, and the dump requests it
//! sends (docs/specs/network-generation.md, 5.4 to 5.6).
//!
//! **Untrusted.** The bytes come from the kernel, but are decoded as
//! untrusted input (coding standards rules 1 to 3): every length is
//! checked against the bytes left before anything it covers is read, as
//! `NLMSG_OK` and `RTA_OK` check, every sum is checked, and nothing is
//! allocated. Unknown message types and attributes are skipped by their
//! length.

#[cfg(test)]
pub(crate) mod build;
#[cfg(test)]
mod tests;

/// `sizeof(struct nlmsghdr)`, which `NLMSG_HDRLEN` rounds to itself.
const HEADER: usize = 16;

/// `sizeof(struct rtattr)`.
const ATTRIBUTE_HEADER: usize = 4;

/// `NLMSG_ALIGNTO` and `RTA_ALIGNTO`.
const ALIGN: usize = 4;

/// `sizeof(struct ifinfomsg)` and `sizeof(struct ifaddrmsg)`.
const LINK_BODY: usize = 16;
const ADDRESS_BODY: usize = 8;

/// The control message types (`<linux/netlink.h>`); `NLMSG_NOOP` (1) is
/// skipped with every other type not read.
const NLMSG_ERROR: u16 = 2;
const NLMSG_DONE: u16 = 3;
const NLMSG_OVERRUN: u16 = 4;

/// The route-netlink message types read (`<linux/rtnetlink.h>`).
const RTM_NEWLINK: u16 = 16;
const RTM_DELLINK: u16 = 17;
const RTM_GETLINK: u16 = 18;
const RTM_NEWADDR: u16 = 20;
const RTM_DELADDR: u16 = 21;
const RTM_GETADDR: u16 = 22;
const RTM_NEWROUTE: u16 = 24;
const RTM_GETROUTE: u16 = 26;
const RTM_NEWRULE: u16 = 32;
const RTM_GETRULE: u16 = 34;
const RTM_NEWNEXTHOP: u16 = 104;
const RTM_GETNEXTHOP: u16 = 106;

/// `NLM_F_REQUEST | NLM_F_DUMP` and `NLM_F_DUMP_INTR`.
const DUMP_FLAGS: u16 = 0x0301;
const NLM_F_DUMP_INTR: u16 = 0x0010;

/// The attributes read: `IFLA_OPERSTATE`; `IFA_ADDRESS`, `IFA_LOCAL`,
/// `IFA_FLAGS`.
const IFLA_OPERSTATE: u16 = 16;
const IFA_ADDRESS: u16 = 1;
const IFA_LOCAL: u16 = 2;
const IFA_FLAGS: u16 = 8;

/// `AF_INET` and `AF_INET6`.
const AF_INET: u8 = 2;
const AF_INET6: u8 = 10;

/// The link flags that make a link usable (`<linux/if.h>`): `IFF_UP`,
/// `IFF_RUNNING`, `IFF_LOWER_UP`, `IFF_DORMANT`.
const USABILITY: u32 = 0x0001 | 0x0040 | 0x0001_0000 | 0x0002_0000;

/// Why a datagram or a message was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Malformed {
    /// Fewer than a header's 16 bytes were left.
    ShortHeader,
    /// A message's length was below its header or past the datagram.
    BadLength,
    /// A message's body was shorter than its fixed header.
    ShortBody,
    /// An attribute's length was below its header or past the body.
    BadAttribute,
    /// A value had the wrong size, or an address the wrong family.
    BadValue,
    /// An address message carried no address.
    NoAddress,
}

/// One message of a datagram, its length checked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Message<'a> {
    pub(crate) kind: u16,
    pub(crate) flags: u16,
    pub(crate) sequence: u32,
    /// The bytes after the header, up to the message's length.
    pub(crate) body: &'a [u8],
}

impl Message<'_> {
    /// Whether the kernel marked the dump inconsistent.
    pub(crate) fn interrupted(&self) -> bool {
        self.flags & NLM_F_DUMP_INTR != 0
    }
}

/// What identifies an address: its family, link, prefix length and bytes
/// (an IPv4 address in the first four).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct AddressKey {
    pub(crate) family: u8,
    pub(crate) index: u32,
    pub(crate) prefix_length: u8,
    pub(crate) address: [u8; 16],
}

/// An address and what decides its usability.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Address {
    pub(crate) key: AddressKey,
    /// `IFA_FLAGS` when present, else the one-byte flags.
    pub(crate) flags: u32,
    pub(crate) scope: u8,
}

/// A link and what decides its usability.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Link {
    pub(crate) index: i32,
    /// The link's flags, only those of `USABILITY`.
    pub(crate) usability: u32,
    pub(crate) operstate: Option<u8>,
}

/// What one message says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Event {
    Link(Link),
    LinkGone(i32),
    Address(Address),
    AddressGone(Address),
    /// A route added, changed or removed.
    Route,
    /// A policy rule added, changed or removed.
    Rule,
    /// A nexthop object added, changed or removed.
    Nexthop,
    /// The end of a dump.
    Done,
    /// `NLMSG_ERROR`, with its (negative or zero) error.
    Error(i32),
    /// `NLMSG_OVERRUN`: data was lost.
    Overrun,
    /// A type this monitor does not read.
    Skipped,
}

/// The dumps a snapshot asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Dump {
    Links,
    Addresses,
}

/// The messages of a datagram. After a refused header it yields nothing
/// more: what follows cannot be found.
#[derive(Debug)]
pub(crate) struct Messages<'a> {
    rest: &'a [u8],
}

impl<'a> Iterator for Messages<'a> {
    type Item = Result<Message<'a>, Malformed>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.rest.is_empty() {
            return None;
        }
        let result = split_message(self.rest);
        match result {
            Ok((_, rest)) => self.rest = rest,
            Err(_) => self.rest = &[],
        }
        Some(result.map(|(message, _)| message))
    }
}

/// The messages of `datagram`.
pub(crate) fn messages(datagram: &[u8]) -> Messages<'_> {
    Messages { rest: datagram }
}

/// The first message of `bytes` and what follows its padding
/// (`NLMSG_OK`, `NLMSG_NEXT`).
fn split_message(bytes: &[u8]) -> Result<(Message<'_>, &[u8]), Malformed> {
    let header = bytes.get(..HEADER).ok_or(Malformed::ShortHeader)?;
    let length = usize::try_from(u32_at(header, 0)?).map_err(|_| Malformed::BadLength)?;
    if length < HEADER || length > bytes.len() {
        return Err(Malformed::BadLength);
    }
    let body = bytes.get(HEADER..length).ok_or(Malformed::BadLength)?;
    let message = Message {
        kind: u16_at(header, 4)?,
        flags: u16_at(header, 6)?,
        sequence: u32_at(header, 8)?,
        body,
    };
    Ok((message, after(bytes, length)))
}

/// What follows the first `length` bytes of `bytes` once padded to 4; empty
/// if the padding passes the end.
fn after(bytes: &[u8], length: usize) -> &[u8] {
    length
        .checked_next_multiple_of(ALIGN)
        .and_then(|next| bytes.get(next..))
        .unwrap_or_default()
}

/// The attributes of `body`, each as its type and value (`RTA_OK`,
/// `RTA_NEXT`).
#[derive(Debug)]
struct Attributes<'a> {
    rest: &'a [u8],
}

impl<'a> Iterator for Attributes<'a> {
    type Item = Result<(u16, &'a [u8]), Malformed>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.rest.is_empty() {
            return None;
        }
        let bytes = self.rest;
        self.rest = &[];
        let Some(header) = bytes.get(..ATTRIBUTE_HEADER) else {
            return Some(Err(Malformed::BadAttribute));
        };
        let parsed = u16_at(header, 0).and_then(|length| Ok((length, u16_at(header, 2)?)));
        let Ok((length, kind)) = parsed else {
            return Some(Err(Malformed::BadAttribute));
        };
        let length = usize::from(length);
        if length < ATTRIBUTE_HEADER || length > bytes.len() {
            return Some(Err(Malformed::BadAttribute));
        }
        let Some(value) = bytes.get(ATTRIBUTE_HEADER..length) else {
            return Some(Err(Malformed::BadAttribute));
        };
        self.rest = after(bytes, length);
        Some(Ok((kind, value)))
    }
}

/// What `message` says.
///
/// # Errors
///
/// [`Malformed`] for a body shorter than its fixed header, a bad
/// attribute, a value of the wrong size, or an address message with no
/// address or another family.
pub(crate) fn event(message: &Message<'_>) -> Result<Event, Malformed> {
    match message.kind {
        RTM_NEWLINK => Ok(Event::Link(link(message.body)?)),
        RTM_DELLINK => Ok(Event::LinkGone(link(message.body)?.index)),
        RTM_NEWADDR => Ok(Event::Address(address(message.body)?)),
        RTM_DELADDR => Ok(Event::AddressGone(address(message.body)?)),
        RTM_NEWROUTE..=RTM_GETROUTE => Ok(Event::Route),
        RTM_NEWRULE..=RTM_GETRULE => Ok(Event::Rule),
        RTM_NEWNEXTHOP..=RTM_GETNEXTHOP => Ok(Event::Nexthop),
        NLMSG_DONE => Ok(Event::Done),
        NLMSG_ERROR => {
            let error = message.body.get(..4).ok_or(Malformed::ShortBody)?;
            Ok(Event::Error(i32::from_ne_bytes(array(error)?)))
        }
        NLMSG_OVERRUN => Ok(Event::Overrun),
        _ => Ok(Event::Skipped),
    }
}

/// A link message's body.
fn link(body: &[u8]) -> Result<Link, Malformed> {
    let fixed = body.get(..LINK_BODY).ok_or(Malformed::ShortBody)?;
    let index = i32::from_ne_bytes(array(fixed.get(4..8).ok_or(Malformed::ShortBody)?)?);
    let flags = u32_at(fixed, 8)?;
    let mut operstate = None;
    let rest = body.get(LINK_BODY..).unwrap_or_default();
    for attribute in (Attributes { rest }) {
        let (kind, value) = attribute?;
        if kind == IFLA_OPERSTATE {
            let [state] = <[u8; 1]>::try_from(value).map_err(|_| Malformed::BadValue)?;
            operstate = Some(state);
        }
    }
    Ok(Link {
        index,
        usability: flags & USABILITY,
        operstate,
    })
}

/// An address message's body.
fn address(body: &[u8]) -> Result<Address, Malformed> {
    let fixed = body.get(..ADDRESS_BODY).ok_or(Malformed::ShortBody)?;
    let [family, prefix_length, short_flags, scope] =
        array(fixed.get(..4).ok_or(Malformed::ShortBody)?)?;
    let size = match family {
        AF_INET => 4,
        AF_INET6 => 16,
        _ => return Err(Malformed::BadValue),
    };
    let index = u32_at(fixed, 4)?;
    let mut flags = u32::from(short_flags);
    let mut prefix = None;
    let mut local = None;
    let rest = body.get(ADDRESS_BODY..).unwrap_or_default();
    for attribute in (Attributes { rest }) {
        let (kind, value) = attribute?;
        match kind {
            IFA_ADDRESS | IFA_LOCAL => {
                if value.len() != size {
                    return Err(Malformed::BadValue);
                }
                let mut bytes = [0_u8; 16];
                bytes
                    .get_mut(..size)
                    .ok_or(Malformed::BadValue)?
                    .copy_from_slice(value);
                if kind == IFA_LOCAL {
                    local = Some(bytes);
                } else {
                    prefix = Some(bytes);
                }
            }
            IFA_FLAGS => flags = u32::from_ne_bytes(array(value).map_err(|_| Malformed::BadValue)?),
            _ => {}
        }
    }
    let address = local.or(prefix).ok_or(Malformed::NoAddress)?;
    Ok(Address {
        key: AddressKey {
            family,
            index,
            prefix_length,
            address,
        },
        flags,
        scope,
    })
}

/// A request to dump every link or every address, of every family, with
/// `sequence` (5.4, point 7).
pub(crate) fn dump_request(kind: Dump, sequence: u32) -> [u8; 20] {
    let message_type = match kind {
        Dump::Links => RTM_GETLINK,
        Dump::Addresses => RTM_GETADDR,
    };
    let mut request = [0_u8; 20];
    let fields = [
        &20_u32.to_ne_bytes()[..],
        &message_type.to_ne_bytes(),
        &DUMP_FLAGS.to_ne_bytes(),
        &sequence.to_ne_bytes(),
    ];
    let mut at = request.iter_mut();
    for byte in fields.into_iter().flatten() {
        if let Some(slot) = at.next() {
            *slot = *byte;
        }
    }
    request
}

/// The `N` bytes of `bytes`, which must be exactly `N` long.
fn array<const N: usize>(bytes: &[u8]) -> Result<[u8; N], Malformed> {
    <[u8; N]>::try_from(bytes).map_err(|_| Malformed::ShortBody)
}

/// The host-order `u16` at `at`.
fn u16_at(bytes: &[u8], at: usize) -> Result<u16, Malformed> {
    let end = at.checked_add(2).ok_or(Malformed::ShortBody)?;
    Ok(u16::from_ne_bytes(array(
        bytes.get(at..end).ok_or(Malformed::ShortBody)?,
    )?))
}

/// The host-order `u32` at `at`.
fn u32_at(bytes: &[u8], at: usize) -> Result<u32, Malformed> {
    let end = at.checked_add(4).ok_or(Malformed::ShortBody)?;
    Ok(u32::from_ne_bytes(array(
        bytes.get(at..end).ok_or(Malformed::ShortBody)?,
    )?))
}
