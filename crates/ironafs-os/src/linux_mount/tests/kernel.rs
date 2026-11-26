// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Real FUSE mount tests in a private mount and PID namespace. The
//! launcher owns the temporary directory outside that namespace and
//! enforces the deadline for the whole process, including the FUSE read.

mod fuse;

use std::fs::{self, OpenOptions};
use std::os::fd::AsFd;
use std::os::unix::fs::MetadataExt;
use std::path::PathBuf;
use std::thread;

use crate::dump::harness::{Command, own_status, status_ids};
use crate::mount_fuse;

/// `CAP_SYS_ADMIN` in `/proc/self/status`'s effective capability mask.
const CAP_SYS_ADMIN: u64 = 1 << 21;
/// `EPERM` from `mount(2)` when the caller lacks mount privilege.
const EPERM: i32 = 1;
/// A directory with mode 0700, as passed in `rootmode`.
const ROOT_MODE: u32 = 0o40700;

/// Requires a new PID and mount namespace with private propagation, checks
/// the expected capability, and returns the private mountpoint and IDs.
fn mount_environment(privileged: bool) -> (PathBuf, u32, u32) {
    assert_eq!(
        std::process::id(),
        1,
        "run as PID 1 in a disposable namespace"
    );
    let outside = std::env::var_os("IRONAFS_OS_TEST_OUTSIDE_MNT_NS").unwrap();
    let inside = fs::read_link("/proc/self/ns/mnt").unwrap();
    assert_ne!(inside.as_os_str(), outside, "mount namespace must be new");
    let mountinfo = fs::read_to_string("/proc/self/mountinfo").unwrap();
    assert_ne!(mountinfo, "", "mountinfo must describe the namespace");
    for line in mountinfo.lines() {
        let (fields, _) = line.split_once(" - ").unwrap();
        assert!(
            fields.split_whitespace().skip(6).all(|tag| {
                !tag.starts_with("shared:")
                    && !tag.starts_with("master:")
                    && !tag.starts_with("propagate_from:")
            }),
            "mount propagation must be private: {line}"
        );
    }
    let status = own_status();
    let effective = status
        .lines()
        .find_map(|line| line.strip_prefix("CapEff:"))
        .unwrap();
    let capable = u64::from_str_radix(effective.trim(), 16).unwrap() & CAP_SYS_ADMIN != 0;
    assert_eq!(capable, privileged, "effective CAP_SYS_ADMIN");
    let uid = status_ids(&status, "Uid:")[1];
    let gid = status_ids(&status, "Gid:")[1];
    if !privileged {
        assert_ne!(uid, 0, "the refusal control must run as a non-root user");
    }

    let path = PathBuf::from(std::env::var_os("IRONAFS_OS_TEST_MOUNT_DIR").unwrap());
    let mountpoint = fs::symlink_metadata(&path).unwrap();
    assert!(path.is_absolute() && mountpoint.is_dir());
    assert_eq!(mountpoint.mode() & 0o777, 0o700);
    assert_eq!(fs::read_dir(&path).unwrap().count(), 0);
    (path, uid, gid)
}

#[test]
#[ignore = "needs /dev/fuse and CAP_SYS_ADMIN in a private mount and PID namespace under a hard timeout"]
fn the_mount_serves_a_getattr_request() {
    let (path, uid, gid) = mount_environment(true);
    let device = OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/fuse")
        .unwrap();
    mount_fuse(&path, device.as_fd(), ROOT_MODE, uid, gid).unwrap();

    // Keep the device open until stat returns. If the server panics, its
    // owned descriptor closes and the kernel aborts the connection.
    let server = thread::spawn(move || {
        fuse::serve_getattr(&device, uid, gid);
        device
    });
    // GNU stat requests the fields in its format. Mode and link count
    // need only GETATTR, even on kernels that can query a birth time via
    // FUSE_STATX.
    let attributes = Command::new("stat")
        .args(["--format=%f %h", "--"])
        .arg(&path)
        .output();
    let _device = server.join().unwrap();
    // The server's permissions differ from the initial rootmode and the
    // underlying empty directory. Verify the attributes Linux exposes.
    let attributes = attributes.unwrap();
    assert!(attributes.status.success(), "{attributes:?}");
    assert_eq!(attributes.stdout, b"41e9 2\n");
}

#[test]
#[ignore = "needs /dev/fuse as a non-root user in a private mount and PID namespace under a hard timeout"]
fn the_mount_without_cap_sys_admin_returns_eperm() {
    let (path, uid, gid) = mount_environment(false);
    let device = OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/fuse")
        .unwrap();
    let error = mount_fuse(&path, device.as_fd(), ROOT_MODE, uid, gid).unwrap_err();
    assert_eq!(error.raw_os_error(), Some(EPERM));
}
