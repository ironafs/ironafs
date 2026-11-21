// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Test support for the dump controls' tests on Linux (ADR 0004, O26,
//! point 5; ADR 0005, sheets 11 and 12): child processes of the test
//! binary that take a role, markers they plant, and the collector their
//! core dumps go to.
//!
//! **Children.** A test runs its own binary again, selecting one ignored
//! entry test by its exact name and a role by the environment variable
//! [`ROLE`]. The entry test does nothing without the variable.
//!
//! **Markers.** A marker is 64 bytes computed from a salt, one byte at a
//! time, straight into the buffer that holds it, so no other copy exists
//! in the child: not in its arguments, its environment, its stack or its
//! binary. The parent computes the same bytes to search for.
//!
//! **Collector.** The marker tests need the machine configured to write
//! each core image to `<dir>/core.<pid>`, where `<dir>` is the environment
//! variable [`CORE_DIR`]: either straight to the file
//! (`kernel.core_pattern = <dir>/core.%p`, with a non-zero core-file limit
//! in the shell that runs the tests) or through a pipe to a program that
//! records every image it is handed
//! (`kernel.core_pattern = |/usr/bin/tee <dir>/core.%p`, with
//! `kernel.core_pipe_limit` above zero, so the kernel waits for the
//! collector to exit before the child is reaped; core(5)). Those tests are
//! ignored by default and run where the machine is so configured.

#![cfg(target_os = "linux")]

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::ExitStatus;

/// The subprocess builder: these tests must run their own binary again,
/// since a core dump ends the process that makes it. A child's clock and
/// randomness are not inputs to any assertion.
#[expect(
    clippy::disallowed_types,
    reason = "rule 7: a dump test needs a child process to dump; the child's clock and \
              randomness are not inputs to any assertion"
)]
pub(crate) type Command = std::process::Command;

/// The environment variable naming a child's role.
pub(crate) const ROLE: &str = "IRONAFS_OS_TEST_ROLE";

/// The environment variable naming the collector's directory.
pub(crate) const CORE_DIR: &str = "IRONAFS_CORE_DIR";

/// A marker's length: long enough that no chance match is plausible.
pub(crate) const MARKER_LEN: usize = 64;

/// The role this process was started with, if it is a child.
pub(crate) fn role() -> Option<String> {
    std::env::var(ROLE).ok()
}

/// One byte of the marker for `salt`: `SplitMix64`'s output function over
/// the salt and the index.
fn marker_byte(salt: u8, index: usize) -> u8 {
    let mut value = (u64::from(salt) << 32) | u64::try_from(index).unwrap();
    value = value.wrapping_add(0x9E37_79B9_7F4A_7C15);
    value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    value ^= value >> 31;
    value.to_le_bytes()[3]
}

/// The marker for `salt`, as the parent searches for it.
pub(crate) fn marker(salt: u8) -> Vec<u8> {
    (0..MARKER_LEN)
        .map(|index| marker_byte(salt, index))
        .collect()
}

/// Writes the marker for `salt` at the start of `buffer`, one byte at a
/// time, so it is computed nowhere else in memory.
pub(crate) fn plant(buffer: &mut [u8], salt: u8) {
    for (index, byte) in buffer.iter_mut().take(MARKER_LEN).enumerate() {
        *byte = std::hint::black_box(marker_byte(salt, index));
    }
}

/// Whether `needle` occurs in `haystack`.
pub(crate) fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty()
        && haystack
            .windows(needle.len())
            .any(|window| window == needle)
}

/// A command that runs this test binary's entry test `entry` (its full
/// path, such as `unix_nodump::tests::child`) with `role`.
pub(crate) fn command(entry: &str, role: &str) -> Command {
    command_at(&std::env::current_exe().unwrap(), entry, role)
}

/// [`command`], running the test binary at `program`.
pub(crate) fn command_at(program: &Path, entry: &str, role: &str) -> Command {
    let mut command = Command::new(program);
    command
        .args([
            entry,
            "--exact",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(ROLE, role)
        .env_remove(CORE_DIR);
    command
}

/// What a child did.
#[derive(Debug)]
pub(crate) struct Run {
    pub(crate) pid: u32,
    pub(crate) status: ExitStatus,
    pub(crate) stdout: String,
}

impl Run {
    /// The value of the first `key=value` line of the child's output.
    pub(crate) fn value(&self, key: &str) -> Option<&str> {
        let prefix = format!("{key}=");
        self.stdout
            .lines()
            .find_map(|line| line.strip_prefix(prefix.as_str()))
    }
}

/// Runs `command` to its end, its output captured.
pub(crate) fn run(command: &mut Command) -> Run {
    let child = command
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let pid = child.id();
    let output = child.wait_with_output().unwrap();
    Run {
        pid,
        status: output.status,
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
    }
}

/// The configured collector.
#[derive(Debug)]
pub(crate) struct Collector {
    dir: PathBuf,
    /// The kernel pipes each image to a program, rather than writing it.
    pub(crate) piped: bool,
}

impl Collector {
    /// The collector this machine is configured with, or a panic saying
    /// what is missing: an unconfigured machine is missing evidence, never
    /// a pass.
    pub(crate) fn from_environment() -> Self {
        let dir = std::env::var(CORE_DIR).unwrap_or_else(|_| {
            panic!("set {CORE_DIR} to the collector's directory (see dump::harness)")
        });
        let pattern = fs::read_to_string("/proc/sys/kernel/core_pattern").unwrap();
        let pattern = pattern.trim_end();
        let file = format!("{dir}/core.%p");
        let piped = if pattern == file {
            assert_ne!(
                core_soft_limit(),
                "0",
                "the core-file limit is 0: raise it (ulimit -c unlimited) in the shell that \
                 runs these tests"
            );
            false
        } else if pattern.starts_with('|') && pattern.ends_with(&format!(" {file}")) {
            let limit = fs::read_to_string("/proc/sys/kernel/core_pipe_limit").unwrap();
            assert_ne!(
                limit.trim(),
                "0",
                "kernel.core_pipe_limit is 0, so the kernel does not wait for the collector"
            );
            true
        } else {
            panic!("kernel.core_pattern is {pattern:?}, not {file:?} or a pipe ending in it");
        };
        Self {
            dir: PathBuf::from(dir),
            piped,
        }
    }

    /// The mode, for a test's message.
    pub(crate) fn mode(&self) -> &'static str {
        if self.piped { "piped" } else { "file" }
    }

    /// The image the child `pid` left, removed from the directory, or
    /// `None` if it left none. A present image must be a whole ELF core
    /// file: every segment its program headers name is in it.
    pub(crate) fn take_image(&self, pid: u32) -> Option<Vec<u8>> {
        let path = self.dir.join(format!("core.{pid}"));
        let image = match fs::read(&path) {
            Ok(image) => image,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return None,
            Err(error) => panic!("{}: {error}", path.display()),
        };
        fs::remove_file(&path).unwrap();
        let end = elf_core_end(&image)
            .unwrap_or_else(|| panic!("{}: not an ELF core file", path.display()));
        assert!(
            image.len() >= end,
            "{}: {} bytes of an image of {end}: truncated",
            path.display(),
            image.len()
        );
        Some(image)
    }
}

/// The soft core-file limit of this process, as `/proc/self/limits`
/// writes it (`0`, a number or `unlimited`).
pub(crate) fn core_soft_limit() -> String {
    core_limits().0
}

/// The soft and hard core-file limits of this process, as
/// `/proc/self/limits` writes them.
pub(crate) fn core_limits() -> (String, String) {
    let limits = fs::read_to_string("/proc/self/limits").unwrap();
    let line = limits
        .lines()
        .find(|line| line.starts_with("Max core file size"))
        .unwrap();
    let mut fields = line
        .trim_start_matches("Max core file size")
        .split_whitespace();
    (
        fields.next().unwrap().to_owned(),
        fields.next().unwrap().to_owned(),
    )
}

/// The end of the last segment of a 64-bit little-endian ELF core file:
/// the length a whole image has.
fn elf_core_end(image: &[u8]) -> Option<usize> {
    let u16_at = |at: usize| Some(u16::from_le_bytes(image.get(at..at + 2)?.try_into().ok()?));
    let u64_at = |at: usize| Some(u64::from_le_bytes(image.get(at..at + 8)?.try_into().ok()?));
    // ELF magic, 64-bit class, little-endian, type ET_CORE.
    if image.get(..6)? != b"\x7fELF\x02\x01" || u16_at(16)? != 4 {
        return None;
    }
    let table = usize::try_from(u64_at(0x20)?).ok()?;
    let entry = usize::from(u16_at(0x36)?);
    let count = usize::from(u16_at(0x38)?);
    let mut end = table + entry * count;
    for index in 0..count {
        let header = table + entry * index;
        let offset = usize::try_from(u64_at(header + 8)?).ok()?;
        let size = usize::try_from(u64_at(header + 0x20)?).ok()?;
        end = end.max(offset + size);
    }
    Some(end)
}
