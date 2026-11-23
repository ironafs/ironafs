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
    Collector, PrivateDir, Run, binary_copy, command, command_at, contains, core_limits, marker,
    plant, role, run, trusted_parent,
};

/// This file's entry test, which a child runs.
const ENTRY: &str = "unix_nodump::tests::kernel::child";

/// SIGABRT, the dumping signal `std::process::abort` raises.
const SIGABRT: i32 = 6;

/// The salt of the marker the dump tests plant.
const SALT: u8 = 11;

/// An unprivileged user and group to change to.
const NOBODY: u32 = 65_534;

/// The user and group id the set-id exec test runs its grandchild under:
/// one that no account or group of the machine has (checked before use,
/// in the files and through the name service), so no other process can run
/// the set-user-id copy, which only this group may execute. It lies below
/// systemd's dynamic range (61184 to 65519), whose ids the files do not
/// list.
const SET_ID_TEST_ID: u32 = 59_421;

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
            // Taking them again is the reapply rule after a transition.
            println!("again={:?}", disable_dumps());
            println!("dumpable-again={}", dumpable().unwrap());
        }
        // Report the real and effective user ids and the attribute this
        // process started with, then take the controls and report it again.
        "report-then-disable" => {
            let (real, effective) = user_ids();
            println!("ids={real} {effective}");
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
                "disable-then-exec-set-id" | "disable-then-exec-as-nobody" => {
                    let (dir, copy) = if role == "disable-then-exec-set-id" {
                        set_id_copy()
                    } else {
                        nobody_copy()
                    };
                    _private = Some(dir);
                    command_at(&copy, ENTRY, "report-then-disable")
                }
                _ => command(ENTRY, "report-then-disable"),
            };
            match role.as_str() {
                "disable-then-exec-as-nobody" => {
                    command.uid(NOBODY).gid(NOBODY);
                }
                "disable-then-exec-set-id" => {
                    command.uid(SET_ID_TEST_ID).gid(SET_ID_TEST_ID);
                }
                _ => {}
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
/// the group [`SET_ID_TEST_ID`], which no account or group of the machine
/// has, may run (see [`binary_copy`]).
fn set_id_copy() -> (PrivateDir, std::path::PathBuf) {
    assert!(
        unused_id(SET_ID_TEST_ID),
        "{SET_ID_TEST_ID} belongs to an account or group here: choose another"
    );
    binary_copy(SET_ID_TEST_ID, 0o4710)
}

/// A copy of this test binary that [`NOBODY`]'s group may run and read, so
/// a child under `nobody` can run it wherever the build left the binary;
/// readable, so its exec is an ordinary one, which leaves the attribute 1
/// (an unreadable program's exec resets it as a set-id one does).
fn nobody_copy() -> (PrivateDir, std::path::PathBuf) {
    binary_copy(NOBODY, 0o750)
}

/// Whether `id` is no account's user or primary group id and no group's
/// id, in `/etc/passwd` and `/etc/group` (fields 3 and 4, and 3), and
/// whether the name service (`getent`, which also asks systemd's and
/// directory services' sources) knows no user or group of that id.
fn unused_id(id: u32) -> bool {
    let id = id.to_string();
    let named = |file: &str, fields: &[usize]| {
        std::fs::read_to_string(file).unwrap().lines().any(|line| {
            let line: Vec<&str> = line.split(':').collect();
            fields
                .iter()
                .any(|field| line.get(*field) == Some(&id.as_str()))
        })
    };
    // `getent` exits 2 when the key is not found.
    let known = |database: &str| {
        let status = crate::dump::harness::Command::new("getent")
            .args([database, id.as_str()])
            .stdout(std::process::Stdio::null())
            .status()
            .unwrap();
        status.code() != Some(2)
    };
    !named("/etc/passwd", &[2, 3])
        && !named("/etc/group", &[2])
        && !known("passwd")
        && !known("group")
}

/// This process's real and effective user ids, from `/proc/self/status`.
fn user_ids() -> (u32, u32) {
    let status = std::fs::read_to_string("/proc/self/status").unwrap();
    let mut ids = status
        .lines()
        .find_map(|line| line.strip_prefix("Uid:"))
        .unwrap()
        .split_whitespace()
        .map(|id| id.parse().unwrap());
    (ids.next().unwrap(), ids.next().unwrap())
}

/// The value `/proc/sys/fs/suid_dumpable` holds: what the kernel sets
/// the attribute to on a set-id exec.
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
fn assert_reset_then_restored(run: &Run, ids: &str, reset_to: &str) {
    assert!(run.status.success(), "{run:?}");
    assert_eq!(run.value("ids"), Some(ids), "{run:?}");
    assert_eq!(run.value("self"), Some("0"), "{run:?}");
    assert_eq!(run.value("before"), Some(reset_to), "{run:?}");
    assert_eq!(run.value("after"), Some("0"), "{run:?}");
}

#[test]
fn a_trusted_parent_is_a_root_owned_directory_only_root_may_change() {
    // `/run` is root's, mode 0755, on Linux and in the guest's containers,
    // as is `/`.
    trusted_parent(std::path::Path::new("/run")).unwrap();
    // A relative path is refused, whatever it names.
    assert!(trusted_parent(std::path::Path::new("run")).is_err());
    // The temporary directory lets any user add entries.
    let shared = std::path::Path::new("/tmp");
    assert!(trusted_parent(shared).is_err(), "{}", shared.display());
    // A symbolic link is never followed.
    let own = PrivateDir::create("ironafs-os-test").unwrap();
    let link = own.path().join("link");
    std::os::unix::fs::symlink("/run", &link).unwrap();
    assert!(trusted_parent(&link).is_err());
}

#[test]
fn the_set_id_test_s_id_belongs_to_no_account_or_group() {
    assert!(unused_id(SET_ID_TEST_ID), "{SET_ID_TEST_ID}");
    // Root's id is in use, as a control.
    assert!(!unused_id(0));
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
    assert_eq!(run.value("again"), Some("Ok(())"), "{run:?}");
    assert_eq!(run.value("dumpable-again"), Some("0"), "{run:?}");
}

#[test]
fn an_exec_resets_the_attribute_to_dumpable_and_disable_dumps_restores_it() {
    let run = run(&mut command(ENTRY, "disable-then-exec"));
    let (real, effective) = user_ids();
    assert_reset_then_restored(&run, &format!("{real} {effective}"), "1");
}

/// An exec as another user: the child is started under `nobody` (the id
/// change is made between fork and exec, by the standard library), from a
/// copy of the test binary that user may run and read, and the
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
    assert_reset_then_restored(&run, &format!("{NOBODY} {NOBODY}"), "1");
}

/// A set-id exec: the child takes the controls (the attribute 0), then
/// starts its child under [`SET_ID_TEST_ID`] from a set-user-id copy of
/// the test binary. That exec sets the attribute to
/// `/proc/sys/fs/suid_dumpable` (prctl(2)); an ordinary exec sets it to 1,
/// as the test above shows, and with no reset the child's 0 would carry
/// over. Only `suid_dumpable` 2 tells the set-id reset from both, so the
/// test refuses to run under any other value rather than pass on one that
/// shows nothing (0 is the default). It never changes that setting itself:
/// the guest that runs it sets 2 (CI's root step does, on its throwaway
/// runner).
#[test]
#[ignore = "needs root, for a set-user-id copy of the test binary, and fs.suid_dumpable 2: run as root on a disposable Linux guest"]
fn a_set_id_exec_resets_the_attribute_to_suid_dumpable_and_disable_dumps_restores_it() {
    assert!(is_root(), "run this test as root");
    assert_eq!(
        suid_dumpable(),
        "2",
        "set fs.suid_dumpable to 2 on the guest first: under 0 the reset cannot be told from \
         the child's own 0 carried over, and under 1 not from an ordinary exec's 1"
    );
    let run = run(&mut command(ENTRY, "disable-then-exec-set-id"));
    // The grandchild ran as the test id with root's effective id, so the
    // set-user-id bit took effect, and its attribute was 2: neither the 0 it
    // would have carried over nor an ordinary exec's 1.
    assert_reset_then_restored(&run, &format!("{SET_ID_TEST_ID} 0"), "2");
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
