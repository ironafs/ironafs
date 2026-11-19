// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for recognition and backup of pre-push hooks.

use std::fs;
use std::io;

use super::{Error, HOOK_HEADER, backup_foreign_hook, is_managed_hook};

#[test]
fn hook_recognizes_its_header_at_the_start() {
    let mut contents = HOOK_HEADER.as_bytes().to_vec();
    contents.extend_from_slice(b"exec cargo xtask check\n");

    assert!(is_managed_hook(&contents));
}

#[test]
fn hook_rejects_foreign_scripts_that_reference_xtask() {
    let mut contents = b"#!/bin/sh\nexec cargo xtask check\n".to_vec();
    assert!(!is_managed_hook(&contents));

    contents.extend_from_slice(HOOK_HEADER.as_bytes());
    assert!(!is_managed_hook(&contents));
}

#[test]
fn forced_backup_moves_foreign_hook_and_refuses_existing_backup() {
    let directory =
        std::env::temp_dir().join(format!("ironafs-xtask-hook-backup-{}", std::process::id()));
    fs::create_dir(&directory).unwrap();
    let hook = directory.join("pre-push");
    let backup = directory.join("pre-push.xtask-backup");

    fs::write(&hook, b"first foreign hook").unwrap();
    assert_eq!(backup_foreign_hook(&hook).unwrap().as_ref(), Some(&backup));
    assert_eq!(fs::read(&backup).unwrap(), b"first foreign hook");
    assert_eq!(
        fs::symlink_metadata(&hook).unwrap_err().kind(),
        io::ErrorKind::NotFound
    );

    fs::write(&hook, b"second foreign hook").unwrap();
    match backup_foreign_hook(&hook).unwrap_err() {
        Error::Io { source, .. } => assert_eq!(source.kind(), io::ErrorKind::AlreadyExists),
        error => panic!("unexpected error: {error}"),
    }
    assert_eq!(fs::read(&hook).unwrap(), b"second foreign hook");
    assert_eq!(fs::read(&backup).unwrap(), b"first foreign hook");

    fs::remove_dir_all(directory).unwrap();
}
