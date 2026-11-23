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
    Collector, NOBODY, PrivateDir, Run, SIGABRT, binary_copy, command, command_at, contains,
    core_limits, marker, nobody_copy, plant, role, run, trusted_parent,
};

/// This file's entry test, which a child runs.
const ENTRY: &str = "unix_nodump::tests::kernel::child";

/// The salt of the marker the dump tests plant.
const SALT: u8 = 11;

/// The user and group id the set-id exec test runs its grandchild under:
/// one that no account or group of the machine has and no running process
/// holds (checked before use, in the files, through the name service and
/// in every process's ids), so no other process can run the set-user-id
/// copy, which only this group may read and execute. A process that gains the id
/// after the check is not ruled out: the test is for a disposable guest. It
/// lies below systemd's dynamic range (61184 to 65519), whose ids the
/// files do not list.
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
        // Take the controls, then start a child through an exec that
        // resets the attribute, and relay what that child reports.
        "disable-then-exec" | "disable-then-exec-as-nobody" => {
            disable_dumps().unwrap();
            println!("self={}", dumpable().unwrap());
            // Held until the grandchild has run, then removed.
            let mut _private = None;
            let mut command = if role == "disable-then-exec-as-nobody" {
                let (dir, copy) = nobody_copy();
                _private = Some(dir);
                let mut command = command_at(&copy, ENTRY, "report-then-disable");
                command.uid(NOBODY).gid(NOBODY);
                command
            } else {
                command(ENTRY, "report-then-disable")
            };
            let grandchild = run(&mut command);
            assert!(grandchild.status.success(), "{grandchild:?}");
            print!("{}", grandchild.stdout);
        }
        // The set-id helper, already under the test id: report its ids,
        // take the controls and check the attribute is 0, then start the
        // set-user-id copy named by SET_ID_COPY with no id change of its
        // own, and relay what that child reports.
        "disable-then-exec-set-id" => {
            // First of all, before it reads SET_ID_COPY or starts
            // anything: only a process whose real and effective user and
            // group ids are all the test id may be the helper. The
            // set-user-id copy (effective uid 0) refuses the role.
            let (real, effective) = user_ids();
            let (real_group, effective_group) = group_ids();
            if [real, effective, real_group, effective_group] != [SET_ID_TEST_ID; 4] {
                println!("refused={real} {effective} {real_group} {effective_group}");
                panic!("the set-id helper runs only as {SET_ID_TEST_ID} throughout");
            }
            println!("helper-ids={real} {effective}");
            disable_dumps().unwrap();
            let held = dumpable().unwrap();
            assert_eq!(held, 0, "the helper's own controls did not hold");
            println!("self={held}");
            let copy = std::env::var_os(SET_ID_COPY).unwrap();
            let grandchild = run(&mut command_at(
                std::path::Path::new(&copy),
                ENTRY,
                "report-then-disable",
            ));
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

/// The environment variable that names the set-user-id copy to the
/// set-id helper.
const SET_ID_COPY: &str = "IRONAFS_OS_TEST_SET_ID_COPY";

/// Two copies of this test binary that only the group [`SET_ID_TEST_ID`],
/// which no account, group or thread of the machine holds (checked first),
/// may read and run (see [`binary_copy`]): one plain, for the helper, and
/// one owned by root and set-user-id. Both are readable to that group, so
/// neither exec is reset for an unreadable program. Making the set-user-id
/// copy readable adds only reading the test binary's bytes; running it,
/// which the group could already do, runs this test binary as root, where
/// the only role that starts a program, the helper, refuses to run unless
/// its real and effective user and group ids are all the test id. So the
/// copy starts nothing on another's behalf; the guest it runs on is
/// disposable all the same.
fn set_id_copies() -> [(PrivateDir, std::path::PathBuf); 2] {
    assert!(
        unused_id(SET_ID_TEST_ID),
        "{SET_ID_TEST_ID} belongs to an account, group or thread here: choose another"
    );
    [
        binary_copy(SET_ID_TEST_ID, 0o750),
        binary_copy(SET_ID_TEST_ID, 0o4750),
    ]
}

/// Whether `id` is no account's user or primary group id and no group's
/// id, in `/etc/passwd` and `/etc/group` (fields 3 and 4, and 3), whether
/// the name service (`getent`, which also asks systemd's and directory
/// services' sources) knows no user or group of that id, and whether no
/// thread of a running process holds it (a process keeps an id whose
/// account entry was removed). Called only by the privileged case, on the
/// disposable guest it is for: the scan is a snapshot taken before the
/// set-id copy is made, so a process that gains the id later is not ruled
/// out; the guest, not this check, is the access boundary.
fn unused_id(id: u32) -> bool {
    let id = id.to_string();
    let named = |file: &str, fields: &[usize]| {
        account_names(&std::fs::read_to_string(file).unwrap(), fields, &id)
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
        && !held_by_a_thread(&id)
}

/// Whether a line of an account file (`text`, colon-separated) has `id` in
/// one of `fields`.
fn account_names(text: &str, fields: &[usize], id: &str) -> bool {
    text.lines().any(|line| {
        let line: Vec<&str> = line.split(':').collect();
        fields.iter().any(|field| line.get(*field) == Some(&id))
    })
}

/// Whether a `status` file's text has `id` among the real, effective,
/// saved or filesystem user or group ids, or the supplementary groups.
fn status_holds(status: &str, id: &str) -> bool {
    status.lines().any(|line| {
        ["Uid:", "Gid:", "Groups:"].iter().any(|key| {
            line.strip_prefix(key)
                .is_some_and(|ids| ids.split_whitespace().any(|held| held == id))
        })
    })
}

/// Whether any thread of any process (`/proc/<pid>/task/<tid>/status`)
/// holds `id`, by [`status_holds`]. A process or thread that exits while it
/// is read is skipped: its directory or status is gone (`NotFound`), or the
/// kernel reports it gone (`ESRCH`). Any other error fails the preflight,
/// as `Collector::take_image` does, rather than leave a thread unchecked.
/// Each status is read as bytes, so a thread name that is not UTF-8 is no
/// error. A snapshot: see [`unused_id`].
fn held_by_a_thread(id: &str) -> bool {
    /// `ESRCH`.
    const ESRCH: i32 = 3;
    /// `None` for what exited while it was read; a panic for anything else.
    fn read<T>(result: std::io::Result<T>, what: &std::path::Path) -> Option<T> {
        match result {
            Ok(value) => Some(value),
            Err(error)
                if error.kind() == std::io::ErrorKind::NotFound
                    || error.raw_os_error() == Some(ESRCH) =>
            {
                None
            }
            Err(error) => panic!("{}: {error}", what.display()),
        }
    }
    let numbered = |path: &std::path::Path| {
        path.file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.bytes().all(|byte| byte.is_ascii_digit()))
    };
    let proc = std::path::Path::new("/proc");
    std::fs::read_dir(proc).unwrap().any(|entry| {
        let Some(process) = read(entry, proc).map(|entry| entry.path()) else {
            return false;
        };
        if !numbered(&process) {
            return false;
        }
        let tasks = process.join("task");
        let Some(threads) = read(std::fs::read_dir(&tasks), &tasks) else {
            return false;
        };
        threads.into_iter().any(|thread| {
            let Some(thread) = read(thread, &tasks) else {
                return false;
            };
            let path = thread.path().join("status");
            read(std::fs::read(&path), &path)
                .is_some_and(|status| status_holds(&String::from_utf8_lossy(&status), id))
        })
    })
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

/// This process's real and effective group ids, from `/proc/self/status`.
fn group_ids() -> (u32, u32) {
    let status = std::fs::read_to_string("/proc/self/status").unwrap();
    let mut ids = status
        .lines()
        .find_map(|line| line.strip_prefix("Gid:"))
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
fn the_account_and_status_readers_find_an_id_in_each_field_they_read() {
    let passwd = "root:x:0:0:root:/root:/bin/sh\nsvc:x:900:901::/:/bin/false\n";
    assert!(account_names(passwd, &[2, 3], "900"));
    assert!(account_names(passwd, &[2, 3], "901"));
    // The home directory field is not read.
    assert!(!account_names("a:x:1:1::900:/bin/sh\n", &[2, 3], "900"));
    assert!(account_names("grp:x:902:\n", &[2], "902"));
    assert!(!account_names("grp:x:902:\n", &[2], "90"));
    let status = "Name:\tx\nUid:\t1\t2\t3\t4\nGid:\t5\t6\t7\t8\nGroups:\t9 10\n";
    for id in ["1", "4", "5", "8", "9", "10"] {
        assert!(status_holds(status, id), "{id}");
    }
    // A number elsewhere in the file, or part of an id, is not held.
    assert!(!status_holds("Pid:\t11\nUid:\t1\t1\t1\t1\n", "11"));
    assert!(!status_holds(status, "0"));
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

/// A set-id exec, with nothing else to reset the attribute. A helper is
/// started under [`SET_ID_TEST_ID`] (the id change, which itself sets the
/// attribute to `suid_dumpable`, happens there, before its own ordinary
/// exec, which sets it to 1). The helper takes the controls and checks the
/// attribute is 0, then execs a set-user-id copy with no id change of its
/// own; its child reports. The exec sets the attribute to
/// `/proc/sys/fs/suid_dumpable` (prctl(2)) and an ordinary one to 1, and
/// with no reset the helper's 0 would carry over. Only `suid_dumpable` 2
/// tells the set-id reset from both, so the test refuses to run under any
/// other value rather than pass on one that shows nothing (0 is the
/// default). It never changes that setting itself: the guest that runs it
/// sets 2 (CI's root step does, on its throwaway runner).
#[test]
#[ignore = "needs root, for a set-user-id copy of the test binary, and fs.suid_dumpable 2: run as root on a disposable Linux guest"]
fn a_set_id_exec_resets_the_attribute_to_suid_dumpable_and_disable_dumps_restores_it() {
    assert!(is_root(), "run this test as root");
    assert_eq!(
        suid_dumpable(),
        "2",
        "set fs.suid_dumpable to 2 on the guest first: under 0 the reset cannot be told from \
         the helper's own 0 carried over, and under 1 not from an ordinary exec's 1"
    );
    let [(_helper_dir, helper), (_copy_dir, copy)] = set_id_copies();
    let mut command = command_at(&helper, ENTRY, "disable-then-exec-set-id");
    command
        .uid(SET_ID_TEST_ID)
        .gid(SET_ID_TEST_ID)
        .env(SET_ID_COPY, &copy);
    let run = run(&mut command);
    // The helper ran under the test id with no set-user-id bit, took the
    // controls (0) and changed no id after; its child ran as the test id
    // with root's effective id, so the set-user-id bit took effect, and its
    // attribute was 2: neither the helper's 0 carried over nor an ordinary
    // exec's 1. So the 2 is the set-id exec's.
    let helper_ids = format!("{SET_ID_TEST_ID} {SET_ID_TEST_ID}");
    assert_eq!(
        run.value("helper-ids"),
        Some(helper_ids.as_str()),
        "{run:?}"
    );
    assert_reset_then_restored(&run, &format!("{SET_ID_TEST_ID} 0"), "2");
}

/// The helper role refuses a process whose ids are not all the test id,
/// before it reads `SET_ID_COPY` or starts anything: here under the test
/// runner's own ids, whoever they are (never the test id, which no account
/// holds). `SET_ID_COPY` names a program that would print if it ran.
#[test]
fn the_set_id_helper_refuses_ids_other_than_the_test_id_before_it_starts_anything() {
    let run = run(command(ENTRY, "disable-then-exec-set-id").env(SET_ID_COPY, "/bin/echo"));
    assert!(!run.status.success(), "{run:?}");
    let (real, effective) = user_ids();
    let (real_group, effective_group) = group_ids();
    let refused = format!("{real} {effective} {real_group} {effective_group}");
    assert_eq!(run.value("refused"), Some(refused.as_str()), "{run:?}");
    assert_eq!(run.value("helper-ids"), None, "{run:?}");
    assert_eq!(run.value("self"), None, "{run:?}");
}

/// The case the refusal is for: the set-user-id copy itself, started under
/// the test id, so its real ids are the test id and its effective uid is
/// root's, is asked to be the helper. It refuses before it starts
/// anything.
#[test]
#[ignore = "needs root, for a set-user-id copy of the test binary: run as root on a disposable Linux guest"]
fn the_set_user_id_copy_refuses_to_be_the_set_id_helper() {
    assert!(is_root(), "run this test as root");
    let [(_helper_dir, helper), (_copy_dir, copy)] = set_id_copies();
    let mut command = command_at(&copy, ENTRY, "disable-then-exec-set-id");
    command
        .uid(SET_ID_TEST_ID)
        .gid(SET_ID_TEST_ID)
        .env(SET_ID_COPY, &helper);
    let run = run(&mut command);
    assert!(!run.status.success(), "{run:?}");
    let refused = format!("{SET_ID_TEST_ID} 0 {SET_ID_TEST_ID} {SET_ID_TEST_ID}");
    assert_eq!(run.value("refused"), Some(refused.as_str()), "{run:?}");
    assert_eq!(run.value("self"), None, "{run:?}");
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
