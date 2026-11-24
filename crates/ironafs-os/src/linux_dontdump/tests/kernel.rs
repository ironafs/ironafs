// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The tests of `linux-dontdump` (ADR 0005, sheet 12) that need the
//! running kernel itself, so Miri does not build them: the call succeeds and the kernel marks the mapping
//! "do not dump" (`dd` in `/proc/self/smaps`, `proc_pid_smaps(5)`); and the
//! marker test of ADR 0004, O26, point 5, with the first layer (the
//! process's dump controls) switched off in the child, so the buffer's
//! exclusion is the only thing that keeps its marker out.

use std::os::unix::process::ExitStatusExt;

use super::super::SecretBuffer;
use crate::dump::harness::{Collector, SIGABRT, command, contains, marker, plant, role, run};

/// This file's entry test, which a child runs.
const ENTRY: &str = "linux_dontdump::tests::kernel::child";

/// The salts of the marker in the secret buffer and of the control marker
/// in ordinary memory beside it.
const SECRET: u8 = 21;
const CONTROL: u8 = 22;

/// The entry test a child runs; it does nothing without a role.
#[test]
#[ignore = "run only as a child of the dump-exclusion test, which selects its role"]
fn child() {
    let Some(role) = role() else {
        return;
    };
    match role.as_str() {
        // No process dump controls: only the buffer's exclusion protects
        // its marker. The control marker sits in ordinary heap memory.
        "secret-abort" => {
            let mut secret = SecretBuffer::new().unwrap();
            plant(secret.as_mut_slice(), SECRET);
            let mut control = vec![0_u8; 4096];
            plant(&mut control, CONTROL);
            std::hint::black_box((&secret, &control));
            std::process::abort();
        }
        other => panic!("unknown role {other}"),
    }
}

/// The `VmFlags` line of the `/proc/self/smaps` entry that holds `address`.
fn vm_flags(address: usize) -> String {
    let smaps = std::fs::read_to_string("/proc/self/smaps").unwrap();
    let mut inside = false;
    for line in smaps.lines() {
        let range = line.split_whitespace().next().unwrap_or_default();
        if let Some((start, end)) = range.split_once('-')
            && let (Ok(start), Ok(end)) = (
                usize::from_str_radix(start, 16),
                usize::from_str_radix(end, 16),
            )
        {
            inside = (start..end).contains(&address);
        } else if inside && let Some(flags) = line.strip_prefix("VmFlags:") {
            return flags.trim().to_owned();
        }
    }
    panic!("no mapping holds {address:#x}");
}

#[test]
fn secret_buffer_new_marks_its_pages_do_not_dump_on_this_kernel() {
    let buffer = SecretBuffer::new().unwrap();
    let start = buffer.as_slice().as_ptr().addr();
    assert_eq!(start % (64 * 1024), 0, "{start:#x}");
    for address in [start, start + SecretBuffer::SIZE - 1] {
        let flags = vm_flags(address);
        assert!(
            flags.split_whitespace().any(|flag| flag == "dd"),
            "{address:#x}: {flags}"
        );
    }
}

#[test]
#[ignore = "needs a core collector (IRONAFS_CORE_DIR, see dump::harness): run on a configured Linux machine"]
fn dump_of_an_unprotected_child_holds_its_control_marker_and_not_its_secret_buffer() {
    let collector = Collector::from_environment();
    let mode = collector.mode();
    let child = run(&mut command(ENTRY, "secret-abort"));
    assert_eq!(child.status.signal(), Some(SIGABRT), "{mode}: {child:?}");
    assert!(child.status.core_dumped(), "{mode}: {child:?}");
    let image = collector
        .take_image(child.pid)
        .unwrap_or_else(|| panic!("{mode}: the child left no image"));
    assert!(
        contains(&image, &marker(CONTROL)),
        "{mode}: the image lacks the control marker, so it shows nothing"
    );
    assert!(
        !contains(&image, &marker(SECRET)),
        "{mode}: the secret buffer's marker is in the image"
    );
}
