// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for recognition, installation, backup and removal of managed hooks.

use std::fs;
use std::io;

use super::{Error, HOOK_HEADER, install_in, is_managed_hook, uninstall_in};

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
fn install_updates_a_managed_hook_with_stale_contents() {
    let directory =
        std::env::temp_dir().join(format!("ironafs-xtask-hook-stale-{}", std::process::id()));
    fs::create_dir(&directory).unwrap();
    let pre_push = directory.join("pre-push");
    fs::write(&pre_push, format!("{HOOK_HEADER}exec cargo xtask check\n")).unwrap();

    let mut output = Vec::new();
    install_in(&directory, false, &mut output).unwrap();
    assert_eq!(
        fs::read_to_string(&pre_push).unwrap(),
        format!("{HOOK_HEADER}exec cargo xtask pre-push \"$1\" \"$2\"\n")
    );
    assert_eq!(output, [] as [u8; 0]);
    assert_eq!(
        fs::read_dir(&directory).unwrap().count(),
        2,
        "only pre-push and commit-msg remain"
    );
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn install_adds_commit_msg_alongside_an_existing_managed_pre_push() {
    let directory =
        std::env::temp_dir().join(format!("ironafs-xtask-hook-install-{}", std::process::id()));
    fs::create_dir(&directory).unwrap();
    let pre_push = directory.join("pre-push");
    let commit_msg = directory.join("commit-msg");
    let existing = format!("{HOOK_HEADER}exec cargo xtask pre-push \"$1\" \"$2\"\n");
    fs::write(&pre_push, &existing).unwrap();

    let mut output = Vec::new();
    install_in(&directory, false, &mut output).unwrap();
    install_in(&directory, false, &mut output).unwrap();
    assert_eq!(fs::read_to_string(&pre_push).unwrap(), existing);
    assert_eq!(
        fs::read_to_string(&commit_msg).unwrap(),
        format!("{HOOK_HEADER}exec cargo xtask trailers commit \"$1\"\n")
    );
    assert_eq!(output, [] as [u8; 0]);

    uninstall_in(&directory).unwrap();
    uninstall_in(&directory).unwrap();
    for path in [pre_push, commit_msg] {
        assert_eq!(
            path.symlink_metadata().unwrap_err().kind(),
            io::ErrorKind::NotFound
        );
    }
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn foreign_hook_blocks_install_and_uninstall_without_overwriting_either_hook() {
    let directory =
        std::env::temp_dir().join(format!("ironafs-xtask-hook-foreign-{}", std::process::id()));
    fs::create_dir(&directory).unwrap();
    let pre_push = directory.join("pre-push");
    let commit_msg = directory.join("commit-msg");
    fs::write(&commit_msg, b"foreign hook").unwrap();

    let mut output = Vec::new();
    assert!(matches!(
        install_in(&directory, false, &mut output),
        Err(Error::ForeignHook(path)) if path == commit_msg
    ));
    assert_eq!(
        pre_push.symlink_metadata().unwrap_err().kind(),
        io::ErrorKind::NotFound
    );
    fs::write(&pre_push, HOOK_HEADER).unwrap();
    assert!(matches!(
        uninstall_in(&directory),
        Err(Error::ForeignHook(path)) if path == commit_msg
    ));
    assert_eq!(fs::read_to_string(&pre_push).unwrap(), HOOK_HEADER);
    assert_eq!(fs::read(&commit_msg).unwrap(), b"foreign hook");
    assert_eq!(output, [] as [u8; 0]);
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn forced_install_moves_both_foreign_hooks_and_refuses_an_existing_backup() {
    let directory =
        std::env::temp_dir().join(format!("ironafs-xtask-hook-backup-{}", std::process::id()));
    fs::create_dir(&directory).unwrap();
    let pre_push = directory.join("pre-push");
    let commit_msg = directory.join("commit-msg");
    let pre_push_backup = directory.join("pre-push.xtask-backup");
    let commit_msg_backup = directory.join("commit-msg.xtask-backup");
    fs::write(&pre_push, b"original pre-push").unwrap();
    fs::write(&commit_msg, b"original commit-msg").unwrap();
    fs::write(&commit_msg_backup, b"existing backup").unwrap();

    let mut output = Vec::new();
    assert!(matches!(
        install_in(&directory, true, &mut output),
        Err(Error::PathIo { path, source, .. })
            if path == commit_msg_backup && source.kind() == io::ErrorKind::AlreadyExists
    ));
    assert_eq!(fs::read(&pre_push).unwrap(), b"original pre-push");
    assert_eq!(fs::read(&commit_msg).unwrap(), b"original commit-msg");
    assert_eq!(fs::read(&commit_msg_backup).unwrap(), b"existing backup");
    assert_eq!(output, [] as [u8; 0]);

    fs::remove_file(&commit_msg_backup).unwrap();
    install_in(&directory, true, &mut output).unwrap();
    for (path, backup, original, command) in [
        (
            pre_push,
            pre_push_backup,
            "original pre-push",
            "exec cargo xtask pre-push \"$1\" \"$2\"\n",
        ),
        (
            commit_msg,
            commit_msg_backup,
            "original commit-msg",
            "exec cargo xtask trailers commit \"$1\"\n",
        ),
    ] {
        let report = String::from_utf8_lossy(&output);
        assert!(report.contains(&format!("moved existing hook to {}", backup.display())));
        assert_eq!(fs::read_to_string(&backup).unwrap(), original);
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            format!("{HOOK_HEADER}{command}")
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            assert_eq!(path.metadata().unwrap().permissions().mode() & 0o777, 0o755);
        }
    }
    uninstall_in(&directory).unwrap();
    assert_eq!(fs::read_dir(&directory).unwrap().count(), 2);
    fs::remove_dir_all(directory).unwrap();
}
