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

use super::{disable_dumps, dumpable};
use crate::dump::harness::{
    Collector, Run, command, command_at, contains, core_limits, marker, plant, role, run,
};

/// This file's entry test, which a child runs.
const ENTRY: &str = "unix_nodump::tests::child";

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
            let mut command = match role.as_str() {
                "disable-then-exec-set-id" => {
                    command_at(&set_id_copy(), ENTRY, "report-then-disable")
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

/// A copy of this test binary, owned by root and set-user-id, in a
/// directory of its own that any user can enter.
fn set_id_copy() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("ironafs-os-set-id-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
    let copy = dir.join("set-id-child");
    std::fs::copy(std::env::current_exe().unwrap(), &copy).unwrap();
    std::fs::set_permissions(&copy, std::fs::Permissions::from_mode(0o4755)).unwrap();
    copy
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
#[cfg_attr(miri, ignore = "a real system call in a child process")]
fn disable_dumps_reads_back_not_dumpable_and_zeroes_both_core_limits() {
    let run = run(&mut command(ENTRY, "disable"));
    assert!(run.status.success(), "{run:?}");
    assert_eq!(run.value("result"), Some("Ok(())"), "{run:?}");
    assert_eq!(run.value("dumpable"), Some("0"), "{run:?}");
    assert_eq!(run.value("core-limit"), Some("0 0"), "{run:?}");
}

#[test]
#[cfg_attr(miri, ignore = "a real system call in a child process")]
fn an_exec_resets_the_attribute_to_dumpable_and_disable_dumps_restores_it() {
    let run = run(&mut command(ENTRY, "disable-then-exec"));
    assert_reset_then_restored(&run, "1");
}

#[test]
#[ignore = "needs root, to change to another user: run as root on a Linux guest"]
fn an_exec_after_an_id_change_resets_the_attribute_and_disable_dumps_restores_it() {
    assert!(is_root(), "run this test as root");
    let run = run(&mut command(ENTRY, "disable-then-exec-as-nobody"));
    // The id change resets it to suid_dumpable; the exec that follows, with
    // real and effective ids equal, makes it 1.
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
