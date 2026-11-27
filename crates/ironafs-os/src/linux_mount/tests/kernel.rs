// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The pathname refusal and the real FUSE mount cases. The latter run in
//! private mount and PID namespaces under a launcher that owns the
//! mountpoint and enforces the deadline for the whole process.

mod fuse;

use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::fs::{self, File, OpenOptions};
use std::io;
use std::os::fd::AsFd;
use std::os::unix::{ffi::OsStrExt, fs::MetadataExt};
use std::path::{Path, PathBuf};
use std::thread;

use crate::dump::harness::{Command, NOBODY, PrivateDir, own_status, status_ids};
use crate::mount_fuse;

/// `CAP_SYS_ADMIN` in `/proc/self/status`'s effective capability mask.
const CAP_SYS_ADMIN: u64 = 1 << 21;
/// `EPERM` from `mount(2)` when the caller lacks mount privilege.
const EPERM: i32 = 1;
/// `ENOENT` and `ENOTDIR` from pathname resolution in `mount(2)`.
const ENOENT: i32 = 2;
const ENOTDIR: i32 = 20;
/// `EINVAL` for a FUSE mount with invalid device or option values.
const EINVAL: i32 = 22;
/// A directory with mode 0700, as passed in `rootmode`.
const ROOT_MODE: u32 = 0o40700;

#[test]
fn a_target_with_nul_is_refused_by_the_wrapper() {
    let device = File::open("/dev/null").unwrap();
    let target = Path::new(OsStr::from_bytes(b"/dev/null\0/trailing"));
    let error = mount_fuse(target, device.as_fd(), ROOT_MODE, 0, 0).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    // A kernel EINVAL would also have kind InvalidInput.
    assert_eq!(error.raw_os_error(), None);
}

fn has_mount_capability(status: &str) -> bool {
    let effective = status
        .lines()
        .find_map(|line| line.strip_prefix("CapEff:"))
        .unwrap();
    u64::from_str_radix(effective.trim(), 16).unwrap() & CAP_SYS_ADMIN != 0
}

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
    assert_eq!(
        has_mount_capability(&status),
        privileged,
        "effective CAP_SYS_ADMIN"
    );
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

fn fuse_device() -> File {
    OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/fuse")
        .unwrap()
}

/// Checks the per-mount and filesystem-specific fields of the mountinfo
/// entry for the test directory (`proc_pid_mountinfo(5)`).
#[track_caller]
fn assert_fuse_mount(mountinfo: &str, target: &Path, uid: u32, gid: u32) {
    let target = target
        .to_str()
        .unwrap()
        .replace('\\', r"\134")
        .replace(' ', r"\040")
        .replace('\t', r"\011")
        .replace('\n', r"\012");
    let (mount_options, filesystem) = mountinfo
        .lines()
        .find_map(|line| {
            let (mount, filesystem) = line.split_once(" - ").unwrap();
            let mut fields = mount.split_ascii_whitespace().skip(4);
            (fields.next() == Some(target.as_str())).then(|| (fields.next().unwrap(), filesystem))
        })
        .unwrap_or_else(|| panic!("missing mountinfo entry for {target}"));
    // Linux can report its default atime policy and the LSM's security
    // labeling marker. Compare the rest as unordered sets so additional
    // caller-controlled options such as noexec or default_permissions fail.
    let per_mount: BTreeSet<_> = mount_options
        .split(',')
        .filter(|&option| option != "relatime")
        .collect();
    assert_eq!(per_mount, BTreeSet::from(["rw", "nosuid", "nodev"]));
    let mut fields = filesystem.split_ascii_whitespace();
    assert_eq!(fields.next(), Some("fuse"));
    assert_eq!(fields.next(), Some("ironafs"));
    let super_options: BTreeSet<_> = fields
        .next()
        .unwrap()
        .split(',')
        .filter(|&option| option != "seclabel")
        .collect();
    assert_eq!(
        super_options,
        BTreeSet::from([
            "rw",
            "allow_other",
            &format!("user_id={uid}"),
            &format!("group_id={gid}")
        ])
    );
}

#[test]
#[ignore = "needs /dev/fuse and CAP_SYS_ADMIN in a private mount and PID namespace under a hard timeout"]
fn the_mount_serves_a_getattr_request() {
    let (path, _, _) = mount_environment(true);
    // Different IDs also expose a swap or a non-decimal mount option.
    let (uid, gid) = (12_345, 23_456);
    let mountpoint = path.canonicalize().unwrap();
    let device = fuse_device();
    mount_fuse(&path, device.as_fd(), ROOT_MODE, uid, gid).unwrap();
    assert_fuse_mount(
        &fs::read_to_string("/proc/self/mountinfo").unwrap(),
        &mountpoint,
        uid,
        gid,
    );

    // Keep the device open until stat returns. If the server panics, its
    // owned descriptor closes and the kernel aborts the connection.
    let server = thread::spawn(move || {
        fuse::serve_getattr(&device, uid, gid);
        device
    });
    // Request the basic attributes. The fixture declines an optional STATX
    // before answering the GETATTR that supplies them.
    let nobody = NOBODY.to_string();
    let attributes = Command::new("setpriv")
        .args([
            "--reuid",
            &nobody,
            "--regid",
            &nobody,
            "--clear-groups",
            "--no-new-privs",
            "--bounding-set=-all",
            "--inh-caps=-all",
            "--ambient-caps=-all",
            "--",
            "stat",
        ])
        .args(["--format=%f %h %u %g", "--"])
        .arg(&mountpoint)
        .output()
        .unwrap();
    // The server's permissions differ from the initial rootmode and the
    // underlying empty directory. Check stat before joining so an early
    // child failure cannot leave the assertion waiting for a request.
    assert!(attributes.status.success(), "{attributes:?}");
    assert_eq!(attributes.stdout, b"41e9 2 12345 23456\n");
    server.join().unwrap();
}

#[test]
#[ignore = "needs /dev/fuse and CAP_SYS_ADMIN in a private mount and PID namespace under a hard timeout"]
fn the_kernel_refuses_invalid_mount_arguments() {
    let (path, uid, gid) = mount_environment(true);
    let null = OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/null")
        .unwrap();
    let error = mount_fuse(&path, null.as_fd(), ROOT_MODE, uid, gid).unwrap_err();
    assert_eq!(
        error.raw_os_error(),
        Some(EINVAL),
        "/dev/null descriptor: {error}"
    );

    for (target, errno) in [
        (path.join("absent"), ENOENT),
        (PathBuf::from("/dev/null/child"), ENOTDIR),
    ] {
        let error = mount_fuse(&target, fuse_device().as_fd(), ROOT_MODE, uid, gid).unwrap_err();
        assert_eq!(
            error.raw_os_error(),
            Some(errno),
            "{}: {error}",
            target.display()
        );
    }

    // 030000 has no Linux file type. The all-ones ID is unmapped, as
    // user_namespaces(7) documents for both the uid and gid maps.
    for (argument, mode, user, group) in [
        ("rootmode", 0o30700, uid, gid),
        ("user_id", ROOT_MODE, u32::MAX, gid),
        ("group_id", ROOT_MODE, uid, u32::MAX),
    ] {
        let error = mount_fuse(&path, fuse_device().as_fd(), mode, user, group).unwrap_err();
        assert_eq!(error.raw_os_error(), Some(EINVAL), "{argument}: {error}");
    }
}

#[test]
#[ignore = "needs /dev/fuse as a non-root user in a private mount and PID namespace under a hard timeout"]
fn the_mount_without_cap_sys_admin_returns_eperm() {
    let (path, uid, gid) = mount_environment(false);
    assert_mount_eperm(&path, &fuse_device(), uid, gid);
}

#[test]
#[ignore = "run in a disposable Linux container without CAP_SYS_ADMIN"]
fn the_mount_call_returns_eperm_in_a_restricted_container() {
    assert!(
        !has_mount_capability(&own_status()),
        "effective CAP_SYS_ADMIN"
    );
    let target = PrivateDir::create("ironafs-denied-mount").unwrap();
    // The container need not expose /dev/fuse. The denial of mount is
    // checked before the kernel can use the open device descriptor.
    let device = File::open("/dev/null").unwrap();
    assert_mount_eperm(target.path(), &device, 0, 0);
}

#[track_caller]
fn assert_mount_eperm(target: &Path, device: &File, uid: u32, gid: u32) {
    let error = mount_fuse(target, device.as_fd(), ROOT_MODE, uid, gid).unwrap_err();
    assert_eq!(error.raw_os_error(), Some(EPERM));
}
