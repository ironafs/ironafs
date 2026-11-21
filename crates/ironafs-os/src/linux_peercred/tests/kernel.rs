// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The tests of `linux-peercred` (ADR 0005, sheet 13) that need the
//! running kernel itself, so Miri does not build them: the credentials of
//! a pair name this process's effective ids; a peer under another uid,
//! connected through a listening socket, is named by its own ids and
//! process id; and the credentials are those of the connection's time.

use std::io::Read;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::os::unix::process::CommandExt;
use std::path::PathBuf;

use super::super::peer_credentials;
use crate::dump::harness::{command, role};

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
    let directory: PathBuf =
        std::env::temp_dir().join(format!("ironafs-peercred-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o777)).unwrap();
    let path = directory.join("socket");
    let _ = std::fs::remove_file(&path);
    let listener = UnixListener::bind(&path).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o777)).unwrap();

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
