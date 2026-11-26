// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Checks the C call's arguments, the pathname guard and the errno path.

use std::ffi::{CString, OsStr};
use std::io;
use std::os::fd::{AsFd, AsRawFd};
use std::os::unix::{ffi::OsStrExt, net::UnixStream};
use std::path::Path;

use super::super::abi::stand_in::{Plan, calls, plan};
use crate::mount_fuse;

#[test]
fn mount_passes_fixed_flags_and_exact_c_strings() {
    // Keep enough descriptors open for octal and decimal to differ.
    let _held: Vec<_> = (0..5).map(|_| UnixStream::pair().unwrap()).collect();
    let (device, _peer) = UnixStream::pair().unwrap();
    assert!(device.as_raw_fd() >= 10);
    let target = c"/mount/raw-\xff";
    plan(Plan {
        target: target.to_owned(),
        options: CString::new(format!(
            "fd={},rootmode=40751,user_id=4294967293,group_id=4294967294,allow_other",
            device.as_raw_fd()
        ))
        .unwrap(),
        status: 0,
        errno: 0,
    });
    let before = calls();
    let path = Path::new(OsStr::from_bytes(target.to_bytes()));
    // Both IDs are valid unsigned values with bit 31 set.
    mount_fuse(path, device.as_fd(), 0o40751, u32::MAX - 2, u32::MAX - 1).unwrap();
    assert_eq!(calls() - before, 1);
}

#[test]
fn a_target_with_nul_is_refused_before_the_call() {
    let (device, _peer) = UnixStream::pair().unwrap();
    let path = Path::new(OsStr::from_bytes(b"/mount/first\0/second"));
    let before = calls();
    let result = mount_fuse(path, device.as_fd(), 0o40700, 0, 0);
    assert_eq!(calls(), before);
    assert_eq!(result.unwrap_err().kind(), io::ErrorKind::InvalidInput);
}

#[test]
fn every_nonzero_result_returns_the_calls_errno() {
    let (device, _peer) = UnixStream::pair().unwrap();
    // Linux reports -1. A positive planted status also exercises the
    // wrapper's contract that every nonzero return is an error.
    for (status, errno) in [(-1, 4), (1, 13)] {
        plan(Plan {
            target: c"/mount".to_owned(),
            options: CString::new(format!(
                "fd={},rootmode=40700,user_id=0,group_id=0,allow_other",
                device.as_raw_fd()
            ))
            .unwrap(),
            status,
            errno,
        });
        let before = calls();
        let result = mount_fuse(Path::new("/mount"), device.as_fd(), 0o40700, 0, 0);
        assert_eq!(calls() - before, 1);
        assert_eq!(result.unwrap_err().raw_os_error(), Some(errno));
    }
}
