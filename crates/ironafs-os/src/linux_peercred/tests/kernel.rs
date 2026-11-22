// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The tests of `linux-peercred` (ADR 0005, sheet 13) that need the
//! running kernel itself, so Miri does not build them: the credentials of
//! a pair name this process's effective ids; a peer under another uid,
//! connected through a listening socket, is named by its own ids and
//! process id; the credentials are those of the connection's time; and
//! sockets that are not connected Unix streams, wrapped as streams, are
//! refused.

use std::io::Read;
use std::os::fd::OwnedFd;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixDatagram, UnixListener, UnixStream};
use std::os::unix::process::CommandExt;
use std::path::PathBuf;

use super::super::peer_credentials;
use crate::dump::harness::{PrivateDir, command, role};
use crate::peer::PeerError;

/// This file's entry test, which a child runs.
const ENTRY: &str = "linux_peercred::tests::kernel::child";

/// The environment variable naming the socket the child connects to.
const SOCKET: &str = "IRONAFS_OS_TEST_SOCKET";

/// An unprivileged user and group.
const NOBODY: u32 = 65_534;

/// The effective user and group ids of this process, from
/// `/proc/self/status` (`Uid:` and `Gid:` give real, effective, saved and
/// filesystem ids).
fn effective_ids() -> (u32, u32) {
    let status = std::fs::read_to_string("/proc/self/status").unwrap();
    let effective = |key: &str| -> u32 {
        status
            .lines()
            .find_map(|line| line.strip_prefix(key))
            .and_then(|ids| ids.split_whitespace().nth(1))
            .unwrap()
            .parse()
            .unwrap()
    };
    (effective("Uid:"), effective("Gid:"))
}

/// The entry test a child runs; it does nothing without a role.
#[test]
#[ignore = "run only as a child of the peer-credential tests, which select its role"]
fn child() {
    let Some(role) = role() else {
        return;
    };
    match role.as_str() {
        // Connect, then wait until the parent closes its end.
        "connect" => {
            let path = std::env::var(SOCKET).unwrap();
            let mut stream = UnixStream::connect(path).unwrap();
            let mut rest = Vec::new();
            stream.read_to_end(&mut rest).unwrap();
        }
        other => panic!("unknown role {other}"),
    }
}

#[test]
fn peer_credentials_of_a_socket_pair_name_this_process_s_effective_ids() {
    let (ours, _theirs) = UnixStream::pair().unwrap();
    let peer = peer_credentials(&ours).unwrap();
    assert_eq!((peer.uid(), peer.gid()), effective_ids());
    assert_eq!(peer.pid(), i32::try_from(std::process::id()).unwrap());
}

#[test]
fn peer_credentials_of_a_shut_down_stream_are_still_the_connection_s() {
    let (ours, theirs) = UnixStream::pair().unwrap();
    theirs.shutdown(std::net::Shutdown::Both).unwrap();
    drop(theirs);
    let peer = peer_credentials(&ours).unwrap();
    assert_eq!(peer.pid(), i32::try_from(std::process::id()).unwrap());
}

#[test]
#[ignore = "needs root, to run a peer under another user: run as root on a Linux guest"]
fn peer_credentials_name_a_peer_under_another_uid() {
    // Created exclusively, so a path planted before (a symlink included)
    // fails the test rather than being followed. The other user needs
    // only to search the directory and to write to the socket.
    let directory: PathBuf =
        std::env::temp_dir().join(format!("ironafs-peercred-{}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o755)).unwrap();
    let path = directory.join("socket");
    let listener = UnixListener::bind(&path).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o666)).unwrap();

    let mut child = command(ENTRY, "connect")
        .env(SOCKET, &path)
        .uid(NOBODY)
        .gid(NOBODY)
        .spawn()
        .unwrap();
    let (stream, _) = listener.accept().unwrap();
    let peer = peer_credentials(&stream).unwrap();
    drop(stream);
    let status = child.wait().unwrap();
    std::fs::remove_dir_all(&directory).unwrap();

    assert!(status.success(), "{status:?}");
    assert_eq!((peer.uid(), peer.gid()), (NOBODY, NOBODY));
    assert_eq!(peer.pid(), i32::try_from(child.id()).unwrap());
    assert_ne!(peer.uid(), effective_ids().0, "run this test as root");
}

#[test]
fn peer_credentials_refuse_a_listener_wrapped_as_a_stream() {
    // Safe Rust can wrap any descriptor as a `UnixStream`; a listener's
    // `SO_PEERCRED` answers with its own credentials.
    let dir = PrivateDir::create("ironafs-peercred").unwrap();
    let listener = UnixListener::bind(dir.path().join("socket")).unwrap();
    let wrapped = UnixStream::from(OwnedFd::from(listener));
    assert!(
        matches!(peer_credentials(&wrapped), Err(PeerError::NotConnected(_))),
        "{:?}",
        peer_credentials(&wrapped)
    );
}

/// `SOCK_DGRAM`, the type a datagram socket reports.
const SOCK_DGRAM: i32 = 2;

#[test]
fn peer_credentials_refuse_sockets_that_are_not_connected_unix_streams() {
    // A connected datagram socket, an unconnected one and a UDP socket,
    // each wrapped as a stream: the first has a Unix peer but is no
    // stream, the others have no Unix peer. None reaches the credential
    // read, so the refusal of ids of -1 is driven through the stand-in
    // (`stand_in`), not here: every connected Unix stream has peer
    // credentials.
    let dir = PrivateDir::create("ironafs-peercred").unwrap();
    let ours = UnixDatagram::bind(dir.path().join("ours")).unwrap();
    let _theirs = UnixDatagram::bind(dir.path().join("theirs")).unwrap();
    ours.connect(dir.path().join("theirs")).unwrap();
    let wrapped = UnixStream::from(OwnedFd::from(ours));
    let found = peer_credentials(&wrapped);
    assert!(
        matches!(found, Err(PeerError::NotStream(SOCK_DGRAM))),
        "a connected datagram socket: {found:?}"
    );
    let udp = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
    for (case, descriptor) in [
        (
            "an unconnected datagram socket",
            OwnedFd::from(UnixDatagram::unbound().unwrap()),
        ),
        ("a UDP socket", OwnedFd::from(udp)),
    ] {
        let wrapped = UnixStream::from(descriptor);
        let found = peer_credentials(&wrapped);
        assert!(
            matches!(found, Err(PeerError::NotConnected(_))),
            "{case}: {found:?}"
        );
    }
}

#[test]
fn peer_credentials_refuse_a_reconnected_datagram_socket_of_a_pair() {
    // A datagram pair has credentials cached when it was made; connected
    // to another peer, its `SO_PEERCRED` still answers the pair's, so only
    // the type check refuses it.
    let dir = PrivateDir::create("ironafs-peercred").unwrap();
    let other = UnixDatagram::bind(dir.path().join("other")).unwrap();
    let (ours, _theirs) = UnixDatagram::pair().unwrap();
    ours.connect(dir.path().join("other")).unwrap();
    let wrapped = UnixStream::from(OwnedFd::from(ours));
    let found = peer_credentials(&wrapped);
    assert!(
        matches!(found, Err(PeerError::NotStream(SOCK_DGRAM))),
        "{found:?}"
    );
    drop(other);
}
