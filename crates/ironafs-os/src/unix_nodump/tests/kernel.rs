// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests of `unix-nodump` on Linux (ADR 0005, sheet 11): the read-back,
//! the read-back after each transition that resets the attribute, and the
//! marker test of ADR 0004, O26, point 5, with its positive control, for
//! a core dump written to a file and one piped to a collector.
//!
//! Every test runs the controls in a child process, so the test process
//! itself stays dumpable and keeps its core-file limit. The steps' logic,
//! every failure included, is `dump`'s, with stand-ins.

use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::{CommandExt, ExitStatusExt};

use super::super::{disable_dumps, dumpable};
use crate::dump::harness::{
    Collector, PrivateDir, Run, command, command_at, contains, core_limits, marker, plant, role,
    run,
};

/// This file's entry test, which a child runs.
const ENTRY: &str = "unix_nodump::tests::kernel::child";

/// SIGABRT, the dumping signal `std::process::abort` raises.
const SIGABRT: i32 = 6;

/// The salt of the marker the dump tests plant.
const SALT: u8 = 11;

/// An unprivileged user and group to change to.
const NOBODY: u32 = 65_534;

/// The entry test a child runs; it does nothing without a role.
#[test]
#[ignore = "run only as a child of the dump-control tests, which select its role"]
fn child() {
    let Some(role) = role() else {
        return;
    };
    // The test harness has written "test <name> ... " with no newline.
    println!();
    match role.as_str() {
        // Take the controls, then report the attribute and the limits.
        "disable" => {
            let result = disable_dumps();
            let (soft, hard) = core_limits();
            println!("result={result:?}");
            println!("dumpable={}", dumpable().unwrap());
            println!("core-limit={soft} {hard}");
        }
        // Report the attribute this process started with, then take the
        // controls and report it again.
        "report-then-disable" => {
            println!("before={}", dumpable().unwrap());
            disable_dumps().unwrap();
            println!("after={}", dumpable().unwrap());
        }
        // Take the controls, then start a child through `how`, an exec
        // that resets the attribute, and relay what that child reports.
        "disable-then-exec" | "disable-then-exec-as-nobody" | "disable-then-exec-set-id" => {
            disable_dumps().unwrap();
            println!("self={}", dumpable().unwrap());
            // Held until the grandchild has run, then removed.
            let mut _private = None;
            let mut command = match role.as_str() {
                "disable-then-exec-set-id" => {
                    let (dir, copy) = set_id_copy();
                    _private = Some(dir);
                    command_at(&copy, ENTRY, "report-then-disable")
                }
                _ => command(ENTRY, "report-then-disable"),
            };
            if role != "disable-then-exec" {
                command.uid(NOBODY).gid(NOBODY);
            }
            let grandchild = run(&mut command);
            assert!(grandchild.status.success(), "{grandchild:?}");
            print!("{}", grandchild.stdout);
        }
        // Plant the marker in ordinary memory and abort, with or without
        // the controls.
        "plain-abort" | "controls-abort" => {
            if role == "controls-abort" {
                disable_dumps().unwrap();
            }
            let mut buffer = vec![0_u8; 4096];
            plant(&mut buffer, SALT);
            std::hint::black_box(&buffer);
            std::process::abort();
        }
        other => panic!("unknown role {other}"),
    }
}

/// A copy of this test binary, owned by root and set-user-id, that only
/// `nobody`'s group may run: in a new private directory (refused if its
/// path exists), which root owns and that group may only enter. The copy is
/// created new, never over an existing file, and the directory is removed,
/// with it, when the returned guard is dropped, on every path.
fn set_id_copy() -> (PrivateDir, std::path::PathBuf) {
    use std::os::unix::fs::{OpenOptionsExt, chown};
    let dir = PrivateDir::create("ironafs-os-set-id").unwrap();
    chown(dir.path(), Some(0), Some(NOBODY)).unwrap();
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o710)).unwrap();
    let copy = dir.path().join("set-id-child");
    let mut source = std::fs::File::open(std::env::current_exe().unwrap()).unwrap();
    let mut target = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o700)
        .open(&copy)
        .unwrap();
    std::io::copy(&mut source, &mut target).unwrap();
    drop(target);
    chown(&copy, Some(0), Some(NOBODY)).unwrap();
    std::fs::set_permissions(&copy, std::fs::Permissions::from_mode(0o4710)).unwrap();
    (dir, copy)
}

/// The value `/proc/sys/fs/suid_dumpable` holds: what the kernel resets
/// the attribute to on an id change or a set-id exec.
fn suid_dumpable() -> String {
    std::fs::read_to_string("/proc/sys/fs/suid_dumpable")
        .unwrap()
        .trim()
        .to_owned()
}

fn is_root() -> bool {
    let status = std::fs::read_to_string("/proc/self/status").unwrap();
    status
        .lines()
        .find_map(|line| line.strip_prefix("Uid:"))
        .unwrap()
        .split_whitespace()
        .all(|id| id == "0")
}

#[track_caller]
fn assert_reset_then_restored(run: &Run, reset_to: &str) {
    assert!(run.status.success(), "{run:?}");
    assert_eq!(run.value("self"), Some("0"), "{run:?}");
    assert_eq!(run.value("before"), Some(reset_to), "{run:?}");
    assert_eq!(run.value("after"), Some("0"), "{run:?}");
}

#[test]
fn a_private_directory_is_new_entered_by_its_owner_alone_and_removed_on_drop() {
    let first = PrivateDir::create("ironafs-os-test").unwrap();
    let path = first.path().to_path_buf();
    let mode = std::fs::metadata(&path).unwrap().permissions().mode();
    assert_eq!(mode & 0o7777, 0o700, "{mode:o}");
    // A path that exists already is refused, never reused.
    assert_eq!(
        PrivateDir::at(path.clone()).unwrap_err().kind(),
        std::io::ErrorKind::AlreadyExists
    );
    std::fs::write(path.join("file"), b"x").unwrap();
    drop(first);
    assert!(!path.exists());
}

#[test]
fn disable_dumps_reads_back_not_dumpable_and_zeroes_both_core_limits() {
    let run = run(&mut command(ENTRY, "disable"));
    assert!(run.status.success(), "{run:?}");
    assert_eq!(run.value("result"), Some("Ok(())"), "{run:?}");
    assert_eq!(run.value("dumpable"), Some("0"), "{run:?}");
    assert_eq!(run.value("core-limit"), Some("0 0"), "{run:?}");
}

#[test]
fn an_exec_resets_the_attribute_to_dumpable_and_disable_dumps_restores_it() {
    let run = run(&mut command(ENTRY, "disable-then-exec"));
    assert_reset_then_restored(&run, "1");
}

/// An exec as another user: the child is started under `nobody` (the id
/// change is made between fork and exec, by the standard library), and the
/// exec that follows, with real and effective ids equal, makes the
/// attribute 1. What the id change alone does, before any exec, is not
/// observed here: safe Rust has no in-process id change, and no site of
/// ADR 0005 makes one (sheet 11), so that is MISSING evidence until a
/// change adds one.
#[test]
#[ignore = "needs root, to change to another user: run as root on a Linux guest"]
fn an_exec_as_another_user_resets_the_attribute_and_disable_dumps_restores_it() {
    assert!(is_root(), "run this test as root");
    let run = run(&mut command(ENTRY, "disable-then-exec-as-nobody"));
    assert_reset_then_restored(&run, "1");
}

#[test]
#[ignore = "needs root, for a set-user-id copy of the test binary: run as root on a Linux guest"]
fn a_set_id_exec_resets_the_attribute_to_suid_dumpable_and_disable_dumps_restores_it() {
    assert!(is_root(), "run this test as root");
    let reset_to = suid_dumpable();
    let run = run(&mut command(ENTRY, "disable-then-exec-set-id"));
    assert_reset_then_restored(&run, &reset_to);
}

#[test]
#[ignore = "needs a core collector (IRONAFS_CORE_DIR, see dump::harness): run on a configured Linux machine"]
fn disable_dumps_leaves_no_image_where_an_unprotected_child_leaves_its_marker() {
    let collector = Collector::from_environment();
    let mode = collector.mode();

    // The positive control: the same child without the controls dumps, and
    // the collector has its marker.
    let plain = run(&mut command(ENTRY, "plain-abort"));
    assert_eq!(plain.status.signal(), Some(SIGABRT), "{mode}: {plain:?}");
    assert!(plain.status.core_dumped(), "{mode}: {plain:?}");
    let image = collector
        .take_image(plain.pid)
        .unwrap_or_else(|| panic!("{mode}: the unprotected child left no image"));
    assert!(
        contains(&image, &marker(SALT)),
        "{mode}: the unprotected child's image lacks its marker"
    );

    // With the controls, no image at all.
    let protected = run(&mut command(ENTRY, "controls-abort"));
    assert_eq!(
        protected.status.signal(),
        Some(SIGABRT),
        "{mode}: {protected:?}"
    );
    assert!(!protected.status.core_dumped(), "{mode}: {protected:?}");
    assert!(
        collector.take_image(protected.pid).is_none(),
        "{mode}: the protected child left an image"
    );
}
