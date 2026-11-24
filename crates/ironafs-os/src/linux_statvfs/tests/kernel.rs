// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The tests of `linux-statvfs` (ADR 0005, sheet 22) that need the running
//! kernel, so Miri does not build them: each answer is compared with
//! coreutils' `stat --file-system` (free blocks for unprivileged users
//! times the fundamental block size, `%a` and `%S`); a filesystem is
//! measured by its own descriptor (`/proc` has nothing free, a tmpfs has);
//! and a descriptor that is not a directory's is refused before the call,
//! with no descriptor left open. A tmpfs of known size, empty and full, is an
//! ignored test run where one is mounted (`IRONAFS_OS_TEST_TMPFS`): its
//! exact readings, and 64 KiB written taken from the answer, are made on a
//! mount no other test writes to.

use crate::dump::harness::Command;
use std::fs::File;
use std::io::Write;
use std::os::fd::AsFd;
use std::os::unix::net::UnixStream;
use std::path::Path;

use super::super::available_space;
use crate::descriptors::open_descriptors;
use crate::space::SpaceError;

/// The path of an empty tmpfs of exactly one mebibyte, for the ignored
/// test of a known size.
const TMPFS: &str = "IRONAFS_OS_TEST_TMPFS";

/// The path of a filesystem that reserves blocks for its superuser (an
/// ext4 made with `-m 5`), for the ignored test of the unprivileged share.
const RESERVED: &str = "IRONAFS_OS_TEST_RESERVED";

/// One mebibyte.
const MIB: u64 = 1 << 20;

/// What `available_space` answers for the directory at `path`.
fn measured(path: &Path) -> u64 {
    let directory = File::open(path).unwrap();
    available_space(directory.as_fd()).unwrap()
}

/// What coreutils' `stat --file-system` says is free to an unprivileged
/// user at `path`: `%a` blocks of `%S` bytes.
fn stat_says(path: &Path) -> u64 {
    stat_free(path, "%a")
}

/// `stat --file-system`'s count `field` (`%a` free to an unprivileged
/// user, `%f` free in all) times the fundamental block size `%S`.
fn stat_free(path: &Path, field: &str) -> u64 {
    let output = Command::new("stat")
        .args(["--file-system", &format!("--format={field} %S")])
        .arg(path)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let text = String::from_utf8(output.stdout).unwrap();
    let mut words = text
        .split_whitespace()
        .map(|word| word.parse::<u64>().unwrap());
    let blocks = words.next().unwrap();
    let size = words.next().unwrap();
    blocks * size
}

#[test]
fn the_answer_for_proc_is_what_stat_says() {
    // `/proc` never changes its free space, so the two readings agree
    // exactly; a shared mount that other tests or processes write to could
    // change between them, so exact readings of a writable filesystem are
    // made on the dedicated tmpfs (the ignored test below).
    let proc = Path::new("/proc");
    assert_eq!(measured(proc), stat_says(proc));
}

#[test]
fn the_answer_for_the_root_and_temporary_directories_is_what_stat_says() {
    // Other processes write to these filesystems while the test runs, so
    // the two readings may differ by what they wrote: far less than the
    // blocks a filesystem reserves for its superuser (`f_bfree` against
    // `f_bavail`, gigabytes on a runner's ext4), which a query of the
    // wrong field would add.
    const SLACK: u64 = 64 * MIB;
    for path in [Path::new("/"), &std::env::temp_dir()] {
        let ours = measured(path);
        let theirs = stat_says(path);
        assert!(
            ours.abs_diff(theirs) <= SLACK,
            "{}: {ours} against {theirs}",
            path.display()
        );
    }
}

#[test]
fn each_descriptor_measures_its_own_filesystem() {
    assert_eq!(measured(Path::new("/proc")), 0);
    assert!(measured(Path::new("/dev/shm")) > 0);
}

#[test]
fn a_descriptor_that_is_not_a_directory_s_is_refused_and_nothing_stays_open() {
    let file = File::open("/proc/self/status").unwrap();
    let (socket, _peer) = UnixStream::pair().unwrap();
    let before = open_descriptors();
    assert!(matches!(
        available_space(file.as_fd()),
        Err(SpaceError::NotDirectory)
    ));
    assert!(matches!(
        available_space(socket.as_fd()),
        Err(SpaceError::NotDirectory)
    ));
    assert_eq!(open_descriptors(), before);
}

#[test]
#[ignore = "needs a filesystem that reserves blocks at IRONAFS_OS_TEST_RESERVED: run on a configured Linux machine"]
fn a_filesystem_that_reserves_blocks_answers_only_the_unprivileged_share() {
    let root = std::env::var(RESERVED).unwrap();
    let root = Path::new(&root);
    let ours = measured(root);
    assert_eq!(ours, stat_says(root));
    assert!(ours < stat_free(root, "%f"), "{ours}");
}

#[test]
#[ignore = "needs an empty tmpfs of one mebibyte at IRONAFS_OS_TEST_TMPFS: run on a configured Linux machine"]
fn a_tmpfs_of_known_size_measures_its_size_empty_and_nothing_full() {
    let root = std::env::var(TMPFS).unwrap();
    let root = Path::new(&root);
    assert_eq!(measured(root), MIB);
    assert_eq!(measured(root), stat_says(root));
    // 64 KiB, a whole number of pages on every page size the targets use,
    // is taken from the answer exactly.
    let mut part = File::create(root.join("part")).unwrap();
    part.write_all(&vec![1_u8; 65_536]).unwrap();
    part.sync_all().unwrap();
    drop(part);
    assert_eq!(measured(root), MIB - 65_536);
    std::fs::remove_file(root.join("part")).unwrap();
    let mut file = File::create(root.join("fill")).unwrap();
    let block = vec![1_u8; 4096];
    while file.write_all(&block).is_ok() {}
    drop(file);
    let full = measured(root);
    assert_eq!(full, stat_says(root));
    assert!(full < 4096, "{full}");
    std::fs::remove_file(root.join("fill")).unwrap();
    assert_eq!(measured(root), MIB);
}
