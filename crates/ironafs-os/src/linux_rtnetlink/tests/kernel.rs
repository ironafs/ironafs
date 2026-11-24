// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The tests of `linux-rtnetlink` (ADR 0005, sheet 23) that need the
//! running kernel, so Miri does not build them, item by item of the
//! sheet's *Test:* paragraph. Unprivileged: a dump answered on the request
//! socket, ending in `NLMSG_DONE`; an address dump of every family
//! returning the host's addresses; a dump datagram read into a buffer too
//! small for it, refused as truncated; a notify socket joins its groups;
//! and the wait: with nothing ready it times out; a wake signalled from
//! another thread ends a blocked wait; a wake signalled before the wait
//! ends it at once; two signals are cleared by one clear; a receive with
//! nothing queued returns "nothing queued" without blocking; a timeout
//! below one millisecond is not passed as 0. In a network namespace of
//! their own, with `CAP_NET_ADMIN` there (ignored tests, run so): an
//! address, a route and a policy rule of each family, and a link change,
//! each seen on the notify socket when added and when removed; a nexthop
//! object, seen when added and when removed; an overflow provoked by a
//! burst the socket does not read, where the wait reports the socket and
//! the receive returns the overflow error; and a datagram another netlink
//! socket sends to the notify socket's group, refused as not the kernel's.
//! The namespace tests share it, and one's burst or forged datagram
//! reaches the others' notify sockets, so they run one at a time
//! (`--test-threads 1`).
//!
//! Every receive here follows a wait that reported the socket, since a
//! receive never waits; "interrupted" and the wait's refused reports are
//! tested through the stand-ins, which return them on demand (point 6.5).
//!
//! The messages are read here with this test's own minimal reader of the
//! netlink header and of the address, route, rule and link messages
//! (`netlink(7)`, `rtnetlink(7)`), which the crate does not have: decoding
//! is outside it. Each change is matched by its type, family, prefix
//! length and address (a link change by its MTU), so an earlier or other
//! event of the same type cannot satisfy it.

use std::fmt::Write as _;
use std::io::Write as _;
use std::thread;
use std::time::Duration;

use super::super::{RouteSocket, Wake};
use crate::clock::OsClock;
use crate::dump::harness::Command;
use crate::linux_clock::read_clock;
use crate::route::{Ready, Receipt, RouteError, Waited};

// The measurements of the specification's section 13, Q5, Q7 to Q9.
mod measure;

/// How long a test waits for what it expects before it fails.
const PATIENCE: Duration = Duration::from_secs(30);

/// The awake clock, in nanoseconds.
fn now() -> u64 {
    read_clock(OsClock::Awake).unwrap()
}

/// The next datagram on `socket`, or the receive's error: each receive
/// follows a wait, on the socket and a wake no one signals, that reported
/// the socket; "nothing queued" waits again.
fn next(socket: &RouteSocket, buffer: &mut [u8]) -> Result<usize, RouteError> {
    let wake = Wake::new().unwrap();
    loop {
        match socket.wait(&wake, PATIENCE).unwrap() {
            Waited::Ready(ready) => assert!(ready.socket() && !ready.woken, "{ready:?}"),
            Waited::TimedOut => panic!("nothing arrived within {PATIENCE:?}"),
            Waited::Interrupted => continue,
        }
        match socket.receive(buffer)? {
            Receipt::Datagram(count) => return Ok(count),
            Receipt::NothingQueued | Receipt::Interrupted => {}
        }
    }
}

/// `struct nlmsghdr`'s length, and the message types and flags used.
const HEADER: usize = 16;
const NLMSG_ERROR: u16 = 2;
const NLMSG_DONE: u16 = 3;
const RTM_NEWLINK: u16 = 16;
const RTM_GETLINK: u16 = 18;
const RTM_NEWADDR: u16 = 20;
const RTM_DELADDR: u16 = 21;
const RTM_GETADDR: u16 = 22;
const RTM_NEWROUTE: u16 = 24;
const RTM_DELROUTE: u16 = 25;
const RTM_NEWRULE: u16 = 32;
const RTM_DELRULE: u16 = 33;
const RTM_NEWNEXTHOP: u16 = 104;
const RTM_DELNEXTHOP: u16 = 105;
/// `NHA_ID`: a nexthop object's id (`<linux/nexthop.h>`).
const NHA_ID: u16 = 1;
const NLM_F_REQUEST: u16 = 1;
const AF_INET: u8 = 2;
const AF_INET6: u8 = 10;
const IFA_ADDRESS: u16 = 1;
const RTA_DST: u16 = 1;
const FRA_SRC: u16 = 2;
const IFLA_MTU: u16 = 4;
const NLM_F_DUMP: u16 = 0x300;

/// A request to dump every link: a header and a `struct rtgenmsg` of
/// family `AF_UNSPEC`, padded to four bytes.
fn dump_links() -> Vec<u8> {
    dump(RTM_GETLINK)
}

/// A request of type `kind` to dump every object of every family, as
/// [`dump_links`].
fn dump(kind: u16) -> Vec<u8> {
    let mut request = Vec::new();
    request.extend_from_slice(&20_u32.to_ne_bytes());
    request.extend_from_slice(&kind.to_ne_bytes());
    request.extend_from_slice(&(NLM_F_REQUEST | NLM_F_DUMP).to_ne_bytes());
    request.extend_from_slice(&1_u32.to_ne_bytes());
    request.extend_from_slice(&0_u32.to_ne_bytes());
    request.extend_from_slice(&[0, 0, 0, 0]);
    request
}

/// The messages of one datagram, each its type and its payload (the
/// bytes after the header, to its length), each message's length checked
/// against what is left of the datagram. The next message starts at the
/// length rounded up to four bytes, or at the datagram's end if that is
/// past it.
fn messages(datagram: &[u8]) -> Vec<(u16, &[u8])> {
    let mut found = Vec::new();
    let mut rest = datagram;
    while rest.len() >= HEADER {
        let length = u32::from_ne_bytes(rest[0..4].try_into().unwrap()) as usize;
        assert!(
            (HEADER..=rest.len()).contains(&length),
            "a message of {length} bytes in {} left",
            rest.len()
        );
        let kind = u16::from_ne_bytes(rest[4..6].try_into().unwrap());
        found.push((kind, &rest[HEADER..length]));
        rest = &rest[((length + 3) & !3).min(rest.len())..];
    }
    found
}

/// The message types in one datagram.
fn types(datagram: &[u8]) -> Vec<u16> {
    messages(datagram)
        .into_iter()
        .map(|(kind, _)| kind)
        .collect()
}

/// One notification, as this test's reader of `rtnetlink(7)`'s messages
/// sees it: its type, its family, the prefix length of the object it names,
/// the object's address (an address's `IFA_ADDRESS`, a route's
/// `RTA_DST`, a rule's `FRA_SRC`), and a link's `IFLA_MTU`.
#[derive(Debug, PartialEq, Eq)]
struct Event {
    kind: u16,
    family: u8,
    length: u8,
    address: Vec<u8>,
    mtu: Option<u32>,
}

/// The events of one datagram, each message's length checked by
/// [`messages`] and each attribute's against what holds it.
fn events(datagram: &[u8]) -> Vec<Event> {
    let mut found = Vec::new();
    for (kind, payload) in messages(datagram) {
        // The fixed part before the attributes, the prefix length's
        // offset in it, and the attribute naming the object.
        let shape = match kind {
            RTM_NEWADDR | RTM_DELADDR => Some((8, 1, IFA_ADDRESS)),
            RTM_NEWROUTE | RTM_DELROUTE => Some((12, 1, RTA_DST)),
            RTM_NEWRULE | RTM_DELRULE => Some((12, 2, FRA_SRC)),
            RTM_NEWLINK => Some((16, 1, IFLA_MTU)),
            RTM_NEWNEXTHOP | RTM_DELNEXTHOP => Some((8, 1, NHA_ID)),
            _ => None,
        };
        if let Some((fixed, prefix, wanted)) = shape {
            assert!(payload.len() >= fixed, "{kind}: {} bytes", payload.len());
            let mut event = Event {
                kind,
                // A nexthop is matched by its id alone.
                family: if matches!(kind, RTM_NEWNEXTHOP | RTM_DELNEXTHOP) {
                    0
                } else {
                    payload[0]
                },
                length: if matches!(kind, RTM_NEWLINK | RTM_NEWNEXTHOP | RTM_DELNEXTHOP) {
                    0
                } else {
                    payload[prefix]
                },
                address: Vec::new(),
                mtu: None,
            };
            let mut attributes = &payload[fixed..];
            while attributes.len() >= 4 {
                let size = usize::from(u16::from_ne_bytes(attributes[0..2].try_into().unwrap()));
                let tag = u16::from_ne_bytes(attributes[2..4].try_into().unwrap()) & 0x3fff;
                assert!(
                    (4..=attributes.len()).contains(&size),
                    "an attribute of {size} bytes"
                );
                let data = &attributes[4..size];
                if tag == wanted && kind == RTM_NEWLINK {
                    event.mtu = Some(u32::from_ne_bytes(data.try_into().unwrap()));
                } else if tag == wanted {
                    event.address = data.to_vec();
                }
                attributes = &attributes[((size + 3) & !3).min(attributes.len())..];
            }
            found.push(event);
        }
    }
    found
}

/// Receives datagrams on `socket` until one holds `wanted`.
fn wait_for(socket: &RouteSocket, wanted: &Event) {
    let mut buffer = vec![0_u8; 32_768];
    loop {
        let count = next(socket, &mut buffer).unwrap();
        if events(&buffer[..count]).contains(wanted) {
            return;
        }
    }
}

/// The event of `kind` for the object of `family` at `address`/`length`.
fn object(kind: u16, family: u8, address: &str, length: u8) -> Event {
    let address = match address.parse::<std::net::IpAddr>().unwrap() {
        std::net::IpAddr::V4(v4) => v4.octets().to_vec(),
        std::net::IpAddr::V6(v6) => v6.octets().to_vec(),
    };
    Event {
        kind,
        family,
        length,
        address,
        mtu: None,
    }
}

/// A link event of lo carrying `mtu`.
fn link_mtu(mtu: u32) -> Event {
    Event {
        kind: RTM_NEWLINK,
        family: 0,
        length: 0,
        address: Vec::new(),
        mtu: Some(mtu),
    }
}

/// The event of `kind` for the nexthop object `id`.
fn nexthop(kind: u16, id: u32) -> Event {
    Event {
        kind,
        family: 0,
        length: 0,
        address: id.to_ne_bytes().to_vec(),
        mtu: None,
    }
}

/// Runs `ip` with `arguments`, which must succeed.
fn ip(arguments: &str) {
    let status = Command::new("ip")
        .args(arguments.split_whitespace())
        .status()
        .unwrap();
    assert!(status.success(), "ip {arguments}");
}

#[test]
fn a_dump_is_answered_on_the_request_socket_ending_in_done() {
    let socket = RouteSocket::request().unwrap();
    socket.send(&dump_links()).unwrap();
    let mut buffer = vec![0_u8; 32_768];
    let mut seen = Vec::new();
    while !seen.contains(&NLMSG_DONE) {
        let count = next(&socket, &mut buffer).unwrap();
        seen.extend(types(&buffer[..count]));
        assert!(!seen.contains(&NLMSG_ERROR), "{seen:?}");
    }
    assert!(seen.contains(&RTM_NEWLINK), "{seen:?}");
}

#[test]
fn an_address_dump_of_every_family_returns_the_host_s_addresses() {
    let socket = RouteSocket::request().unwrap();
    socket.send(&dump(RTM_GETADDR)).unwrap();
    let mut buffer = vec![0_u8; 32_768];
    let mut seen = Vec::new();
    while !seen.contains(&NLMSG_DONE) {
        let count = next(&socket, &mut buffer).unwrap();
        seen.extend(types(&buffer[..count]));
        assert!(!seen.contains(&NLMSG_ERROR), "{seen:?}");
    }
    assert!(seen.contains(&RTM_NEWADDR), "{seen:?}");
}

#[test]
fn a_dump_datagram_larger_than_the_buffer_is_truncated() {
    let socket = RouteSocket::request().unwrap();
    socket.send(&dump_links()).unwrap();
    let mut buffer = [0_u8; 16];
    assert!(matches!(
        next(&socket, &mut buffer),
        Err(RouteError::Truncated)
    ));
}

#[test]
fn a_notify_socket_joins_its_groups_without_privilege() {
    RouteSocket::notify().unwrap();
}

#[test]
fn a_wait_with_nothing_ready_times_out() {
    let socket = RouteSocket::request().unwrap();
    let wake = Wake::new().unwrap();
    let before = now();
    assert_eq!(
        socket.wait(&wake, Duration::from_millis(50)).unwrap(),
        Waited::TimedOut
    );
    assert!(now() - before >= 50_000_000);
}

#[test]
fn a_wake_signalled_from_another_thread_ends_a_blocked_wait() {
    let socket = RouteSocket::request().unwrap();
    let wake = Wake::new().unwrap();
    let before = now();
    thread::scope(|scope| {
        scope.spawn(|| {
            thread::sleep(Duration::from_millis(100));
            wake.signal().unwrap();
        });
        assert_eq!(
            socket.wait(&wake, PATIENCE).unwrap(),
            Waited::Ready(Ready {
                woken: true,
                ..Ready::default()
            })
        );
    });
    let waited = now() - before;
    assert!(waited >= 100_000_000, "{waited}");
    assert!(waited < 10_000_000_000, "{waited}");
}

#[test]
fn a_wake_signalled_before_the_wait_ends_it_at_once_and_the_wait_clears_nothing() {
    let socket = RouteSocket::request().unwrap();
    let wake = Wake::new().unwrap();
    wake.signal().unwrap();
    let before = now();
    for _ in 0..2 {
        assert_eq!(
            socket.wait(&wake, PATIENCE).unwrap(),
            Waited::Ready(Ready {
                woken: true,
                ..Ready::default()
            })
        );
    }
    assert!(now() - before < 10_000_000_000);
}

#[test]
fn two_signals_are_cleared_by_one_clear() {
    let socket = RouteSocket::request().unwrap();
    let wake = Wake::new().unwrap();
    assert!(!wake.clear().unwrap());
    wake.signal().unwrap();
    wake.signal().unwrap();
    assert!(wake.clear().unwrap());
    assert!(!wake.clear().unwrap());
    assert_eq!(
        socket.wait(&wake, Duration::from_millis(10)).unwrap(),
        Waited::TimedOut
    );
}

#[test]
fn a_receive_with_nothing_queued_returns_nothing_queued_without_blocking() {
    let socket = RouteSocket::request().unwrap();
    let mut buffer = [0_u8; 64];
    let before = now();
    assert_eq!(socket.receive(&mut buffer).unwrap(), Receipt::NothingQueued);
    assert!(now() - before < 10_000_000_000);
}

#[test]
fn a_timeout_below_one_millisecond_is_not_passed_as_0() {
    // A 0 would return at once; one millisecond, the rounded-up value,
    // waits at least that long (poll(2) rounds the timeout up).
    let socket = RouteSocket::request().unwrap();
    let wake = Wake::new().unwrap();
    let before = now();
    assert_eq!(
        socket.wait(&wake, Duration::from_nanos(1)).unwrap(),
        Waited::TimedOut
    );
    assert!(now() - before >= 1_000_000);
}

#[test]
#[ignore = "needs CAP_NET_ADMIN in a network namespace of its own, and iproute2: run there, one test at a time"]
fn each_change_of_link_address_route_and_rule_is_seen_on_the_notify_socket() {
    ip("link set lo up");
    let socket = RouteSocket::notify().unwrap();
    for (add, delete, added, deleted) in [
        (
            "address add 2001:db8:1::1/64 dev lo",
            "address del 2001:db8:1::1/64 dev lo",
            object(RTM_NEWADDR, AF_INET6, "2001:db8:1::1", 64),
            object(RTM_DELADDR, AF_INET6, "2001:db8:1::1", 64),
        ),
        (
            "address add 192.0.2.1/24 dev lo",
            "address del 192.0.2.1/24 dev lo",
            object(RTM_NEWADDR, AF_INET, "192.0.2.1", 24),
            object(RTM_DELADDR, AF_INET, "192.0.2.1", 24),
        ),
        (
            "-6 route add 2001:db8:2::/64 dev lo",
            "-6 route del 2001:db8:2::/64 dev lo",
            object(RTM_NEWROUTE, AF_INET6, "2001:db8:2::", 64),
            object(RTM_DELROUTE, AF_INET6, "2001:db8:2::", 64),
        ),
        (
            "route add 198.51.100.0/24 dev lo",
            "route del 198.51.100.0/24 dev lo",
            object(RTM_NEWROUTE, AF_INET, "198.51.100.0", 24),
            object(RTM_DELROUTE, AF_INET, "198.51.100.0", 24),
        ),
        (
            "-6 rule add from 2001:db8:3::/64 table 100",
            "-6 rule del from 2001:db8:3::/64 table 100",
            object(RTM_NEWRULE, AF_INET6, "2001:db8:3::", 64),
            object(RTM_DELRULE, AF_INET6, "2001:db8:3::", 64),
        ),
        (
            "rule add from 203.0.113.0/24 table 100",
            "rule del from 203.0.113.0/24 table 100",
            object(RTM_NEWRULE, AF_INET, "203.0.113.0", 24),
            object(RTM_DELRULE, AF_INET, "203.0.113.0", 24),
        ),
        (
            "link set lo mtu 1500",
            "link set lo mtu 65536",
            link_mtu(1500),
            link_mtu(65536),
        ),
    ] {
        ip(add);
        wait_for(&socket, &added);
        ip(delete);
        wait_for(&socket, &deleted);
    }
}

#[test]
#[ignore = "needs CAP_NET_ADMIN in a network namespace of its own, and iproute2: run there, one test at a time"]
fn a_nexthop_object_added_and_removed_is_seen_on_the_notify_socket() {
    ip("link set lo up");
    let socket = RouteSocket::notify().unwrap();
    ip("nexthop add id 7 dev lo");
    wait_for(&socket, &nexthop(RTM_NEWNEXTHOP, 7));
    ip("nexthop del id 7");
    wait_for(&socket, &nexthop(RTM_DELNEXTHOP, 7));
}

#[test]
#[ignore = "needs CAP_NET_ADMIN in a network namespace of its own, and iproute2: run there, one test at a time"]
fn a_burst_the_socket_does_not_read_is_an_overflow() {
    ip("link set lo up");
    let socket = RouteSocket::notify().unwrap();
    // The batch goes to `ip -batch -` on its standard input: no file, so
    // no path another user could have prepared.
    let mut lines = String::new();
    for index in 0..4000 {
        writeln!(lines, "address add 2001:db8:9::{index:x}/128 dev lo").unwrap();
    }
    for index in 0..4000 {
        writeln!(lines, "address del 2001:db8:9::{index:x}/128 dev lo").unwrap();
    }
    let mut child = Command::new("ip")
        .args(["-batch", "-"])
        .stdin(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(lines.as_bytes())
        .unwrap();
    assert!(child.wait().unwrap().success(), "ip -batch -");
    // The writer has exited, so every notification of the burst is queued
    // or dropped: the wait reports the socket at once, and a drain that
    // finds nothing queued before the overflow error has missed it.
    let wake = Wake::new().unwrap();
    assert!(matches!(
        socket.wait(&wake, PATIENCE).unwrap(),
        Waited::Ready(ready) if ready.socket() && !ready.woken
    ));
    let mut buffer = vec![0_u8; 32_768];
    let overflowed = loop {
        match socket.receive(&mut buffer) {
            Ok(Receipt::Datagram(_) | Receipt::Interrupted) => {}
            Ok(Receipt::NothingQueued) => {
                panic!("the burst of 8000 changes drained without an overflow (ENOBUFS)")
            }
            Err(RouteError::Overflow(error)) => break error,
            Err(other) => panic!("{other:?}"),
        }
    };
    assert_eq!(overflowed.raw_os_error(), Some(105));
}

#[test]
#[ignore = "needs CAP_NET_ADMIN in a network namespace of its own, and python3: run there, one test at a time"]
fn a_datagram_another_netlink_socket_sends_to_the_group_is_not_the_kernel_s() {
    let socket = RouteSocket::notify().unwrap();
    // A netlink header of a link message, sent to the links group from a
    // socket of its own, as only CAP_NET_ADMIN may (`netlink(7)`).
    let sender = "import socket, struct\n\
                  s = socket.socket(16, 3, 0)\n\
                  s.bind((0, 0))\n\
                  s.sendto(struct.pack('=IHHII', 16, 16, 0, 0, 0), (0, 1))\n";
    let status = Command::new("python3")
        .args(["-c", sender])
        .status()
        .unwrap();
    assert!(status.success());
    let mut buffer = vec![0_u8; 4096];
    assert!(matches!(
        next(&socket, &mut buffer),
        Err(RouteError::NotKernel(port)) if port != 0
    ));
}
