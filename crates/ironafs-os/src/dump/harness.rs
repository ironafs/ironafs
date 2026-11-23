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
//! records every image it is handed, copying its standard input to that one
//! file (`kernel.core_pattern = |<collector> <dir>/core.%p`, the collector
//! a script `exec cat > "$1"`; not `tee`, whose standard output the kernel
//! leaves closed, so the image file can take its place and be written
//! twice; with
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

/// SIGABRT, the dumping signal `std::process::abort` raises.
pub(crate) const SIGABRT: i32 = 6;

/// The unprivileged user and group `nobody`, for a child under another
/// user.
pub(crate) const NOBODY: u32 = 65_534;

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
    /// file, exactly: every segment its program headers name is in it, and
    /// nothing after the last.
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
        // Exactly that long: shorter is truncated, longer is a collector
        // that wrote some of it twice.
        assert_eq!(
            image.len(),
            end,
            "{}: {} bytes where the image's headers give {end}",
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

/// The size of the ELF64 file header, of a program header and of a section
/// header.
const ELF_HEADER: usize = 64;
const PROGRAM_HEADER: usize = 56;
const SECTION_HEADER: usize = 64;

/// `PN_XNUM`: the `e_phnum` that says the count is in section zero's
/// `sh_info`, used only for counts of at least this.
const PN_XNUM: u16 = 0xffff;

/// The end of the last part of a 64-bit little-endian ELF core file (its
/// headers, its segments, and the section header table of extended
/// numbering): the length a whole image has. `None` for what is not such
/// a file: another class, byte order or type, entry sizes other than
/// ELF64's, a first section header that is not the null one extended
/// numbering uses, an extended count below `PN_XNUM`, a nonempty header
/// table that starts inside the ELF header or ends outside the image, or
/// arithmetic that overflows.
fn elf_core_end(image: &[u8]) -> Option<usize> {
    let bytes = |at: usize, length: usize| image.get(at..at.checked_add(length)?);
    let u16_at = |at: usize| Some(u16::from_le_bytes(bytes(at, 2)?.try_into().ok()?));
    let u32_at = |at: usize| Some(u32::from_le_bytes(bytes(at, 4)?.try_into().ok()?));
    let u64_at =
        |at: usize| usize::try_from(u64::from_le_bytes(bytes(at, 8)?.try_into().ok()?)).ok();
    // ELF magic, 64-bit class, little-endian, type ET_CORE.
    if image.get(..6)? != b"\x7fELF\x02\x01" || u16_at(16)? != 4 {
        return None;
    }
    if usize::from(u16_at(0x36)?) != PROGRAM_HEADER {
        return None;
    }
    let table = u64_at(0x20)?;
    // The section header table: with more program headers than `e_phnum`
    // can hold, Linux writes `PN_XNUM` (0xffff) there and the real count in
    // the first section header's `sh_info`, in a section header table it
    // appends; that first header is otherwise null (`sh_type` 0).
    let sections = u64_at(0x28)?;
    let section_count = usize::from(u16_at(0x3c)?);
    if section_count != 0 && usize::from(u16_at(0x3a)?) != SECTION_HEADER {
        return None;
    }
    let sections_end = if section_count == 0 {
        0
    } else {
        sections.checked_add(SECTION_HEADER.checked_mul(section_count)?)?
    };
    if section_count != 0 && sections < ELF_HEADER {
        return None;
    }
    let count = match u16_at(0x38)? {
        PN_XNUM => {
            if section_count == 0 || u32_at(sections.checked_add(4)?)? != 0 {
                return None;
            }
            let extended = u32_at(sections.checked_add(0x2c)?)?;
            if extended < u32::from(PN_XNUM) {
                return None;
            }
            usize::try_from(extended).ok()?
        }
        count => usize::from(count),
    };
    if count != 0 && table < ELF_HEADER {
        return None;
    }
    let table_end = table.checked_add(PROGRAM_HEADER.checked_mul(count)?)?;
    if table_end > image.len() || sections_end > image.len() {
        return None;
    }
    let mut end = table_end.max(sections_end);
    for index in 0..count {
        let header = table.checked_add(PROGRAM_HEADER.checked_mul(index)?)?;
        let offset = u64_at(header.checked_add(8)?)?;
        let size = u64_at(header.checked_add(0x20)?)?;
        end = end.max(offset.checked_add(size)?);
    }
    Some(end)
}

/// A directory made for one test, entered by its owner alone, and removed
/// with everything in it when dropped, on every path out of the test.
#[derive(Debug)]
pub(crate) struct PrivateDir {
    path: PathBuf,
}

impl PrivateDir {
    /// A new directory under the system's temporary directory, named as
    /// [`Self::named`] names it.
    pub(crate) fn create(prefix: &str) -> io::Result<Self> {
        Self::named(&std::env::temp_dir(), prefix)
    }

    /// A new directory in `parent`, which must be a trusted parent (see
    /// [`trusted_parent`]), named as [`Self::named`] names it.
    pub(crate) fn create_in(parent: &Path, prefix: &str) -> io::Result<Self> {
        trusted_parent(parent)?;
        Self::named(parent, prefix)
    }

    /// A new directory in `parent`, its name `prefix`, this process's id
    /// and a count of this process's directories, so no two are the same.
    fn named(parent: &Path, prefix: &str) -> io::Result<Self> {
        use std::sync::atomic::{AtomicU32, Ordering};
        static COUNT: AtomicU32 = AtomicU32::new(0);
        let count = COUNT.fetch_add(1, Ordering::Relaxed);
        Self::at(parent.join(format!("{prefix}-{}-{count}", std::process::id())))
    }

    /// A new directory at `path`, mode 0700.
    pub(crate) fn at(path: PathBuf) -> io::Result<Self> {
        use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
        // Not recursive: a path that exists already, whoever made it, is
        // refused, so nothing placed there in advance is used.
        fs::DirBuilder::new().mode(0o700).create(&path)?;
        let made = Self { path };
        fs::set_permissions(&made.path, fs::Permissions::from_mode(0o700))?;
        Ok(made)
    }

    /// The directory's path.
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for PrivateDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

/// Checks that `path` is an absolute path to a directory only root may
/// change, as is every directory above it: none a symbolic link, each
/// owned by root and writable by no group or other user. Entries in it
/// cannot then be added, removed or replaced by anyone else, nor can any
/// directory on its path.
pub(crate) fn trusted_parent(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::MetadataExt;
    if !path.is_absolute() {
        return Err(io::Error::other(format!(
            "{}: not an absolute path",
            path.display()
        )));
    }
    for directory in path.ancestors() {
        let metadata = fs::symlink_metadata(directory)?;
        let trusted =
            metadata.file_type().is_dir() && metadata.uid() == 0 && metadata.mode() & 0o022 == 0;
        if !trusted {
            return Err(io::Error::other(format!(
                "{}: not a directory that root owns and only root may write",
                directory.display()
            )));
        }
    }
    Ok(())
}

/// The directories, each one only root may change, that a copy of the test
/// binary may be made in: the first on a mount that honours set-user-id
/// bits and allows execution (`/run` is often mounted `nosuid` and
/// `noexec`).
const COPY_PARENTS: &[&str] = &["/run", "/var/lib", "/usr/local", "/opt"];

/// A copy of this test binary, owned by root with group `group` and mode
/// `mode`, for a child under another user to run: the binary where the
/// build left it may be out of that user's reach (a checkout under a home
/// directory only its owner may search). It is made in a new directory
/// (refused if its path exists) in the first of [`COPY_PARENTS`] that only
/// root may change (checked) and whose mount honours set-user-id bits and
/// allows execution; root owns the directory and only `group` may enter
/// it. The copy is created new, never over an existing file, and the
/// directory is removed, with it, when the returned guard is dropped, on
/// every path that unwinds. Only root can make one.
pub(crate) fn binary_copy(group: u32, mode: u32) -> (PrivateDir, PathBuf) {
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt, chown};
    let parent = COPY_PARENTS
        .iter()
        .map(Path::new)
        .find(|parent| trusted_parent(parent).is_ok() && exec_mount(parent))
        .expect("no trusted directory on a mount without nosuid and noexec");
    let dir = PrivateDir::create_in(parent, "ironafs-os-test-binary").unwrap();
    chown(dir.path(), Some(0), Some(group)).unwrap();
    fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o710)).unwrap();
    let copy = dir.path().join("test-binary");
    let mut source = fs::File::open(std::env::current_exe().unwrap()).unwrap();
    let mut target = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o700)
        .open(&copy)
        .unwrap();
    io::copy(&mut source, &mut target).unwrap();
    drop(target);
    chown(&copy, Some(0), Some(group)).unwrap();
    fs::set_permissions(&copy, fs::Permissions::from_mode(mode)).unwrap();
    (dir, copy)
}

/// A copy of this test binary that [`NOBODY`]'s group may run and read
/// (see [`binary_copy`]), so a child under `nobody` can run it wherever the
/// build left the binary. Readable, so its exec is an ordinary one, which
/// leaves the dumpable attribute 1 (an unreadable program's exec resets it
/// as a set-id one does).
pub(crate) fn nobody_copy() -> (PrivateDir, PathBuf) {
    binary_copy(NOBODY, 0o750)
}

/// Whether the mount `path` is on honours set-user-id bits and allows
/// execution: the longest mount point of `/proc/self/mountinfo` above it
/// has neither `nosuid` nor `noexec` among its options.
fn exec_mount(path: &Path) -> bool {
    let mounts = fs::read_to_string("/proc/self/mountinfo").unwrap();
    mounts
        .lines()
        .filter_map(|line| {
            let fields: Vec<&str> = line.split(' ').collect();
            let point = Path::new(*fields.get(4)?);
            let options = *fields.get(5)?;
            path.starts_with(point).then_some((point, options))
        })
        .max_by_key(|(point, _)| point.as_os_str().len())
        .is_some_and(|(_, options)| {
            !options
                .split(',')
                .any(|option| option == "nosuid" || option == "noexec")
        })
}

/// A 64-bit little-endian ELF core image: a header, `segments` program
/// headers (each a 16-byte segment after the table), and, with
/// `extended`, the program header count in a section header at the end
/// (`e_phnum` = `PN_XNUM`), as Linux writes it for `PN_XNUM` segments or
/// more.
#[cfg(test)]
fn core_image(segments: u32, extended: bool) -> Vec<u8> {
    let count = usize::try_from(segments).unwrap();
    let table = ELF_HEADER;
    let data = table + PROGRAM_HEADER * count;
    let mut image = vec![0_u8; data + 16 * count];
    image[..6].copy_from_slice(b"\x7fELF\x02\x01");
    image[16..18].copy_from_slice(&4_u16.to_le_bytes());
    image[0x20..0x28].copy_from_slice(&u64::try_from(table).unwrap().to_le_bytes());
    image[0x36..0x38].copy_from_slice(&u16::try_from(PROGRAM_HEADER).unwrap().to_le_bytes());
    let phnum = if extended {
        PN_XNUM
    } else {
        u16::try_from(segments).unwrap()
    };
    image[0x38..0x3a].copy_from_slice(&phnum.to_le_bytes());
    for index in 0..count {
        let header = table + PROGRAM_HEADER * index;
        let offset = u64::try_from(data + 16 * index).unwrap();
        image[header + 8..header + 16].copy_from_slice(&offset.to_le_bytes());
        image[header + 0x20..header + 0x28].copy_from_slice(&16_u64.to_le_bytes());
    }
    if extended {
        let section = image.len();
        image.extend_from_slice(&[0_u8; SECTION_HEADER]);
        image[0x28..0x30].copy_from_slice(&u64::try_from(section).unwrap().to_le_bytes());
        image[0x3a..0x3c].copy_from_slice(&u16::try_from(SECTION_HEADER).unwrap().to_le_bytes());
        image[0x3c..0x3e].copy_from_slice(&1_u16.to_le_bytes());
        image[section + 0x2c..section + 0x30].copy_from_slice(&segments.to_le_bytes());
    }
    image
}

/// The smallest count extended numbering is used for.
#[cfg(test)]
const EXTENDED: u32 = PN_XNUM as u32;

/// Where the section header table of an extended-numbering image starts.
#[cfg(test)]
fn section_of(image: &[u8]) -> usize {
    usize::try_from(u64::from_le_bytes(image[0x28..0x30].try_into().unwrap())).unwrap()
}

#[test]
fn elf_core_end_is_the_end_of_the_last_segment() {
    let image = core_image(3, false);
    assert_eq!(elf_core_end(&image), Some(image.len()));
}

#[test]
fn elf_core_end_counts_an_extended_numbering_section_header() {
    let image = core_image(EXTENDED, true);
    assert_eq!(elf_core_end(&image), Some(image.len()));
}

/// [`core_image`] with extended numbering, whose last segment's data is
/// moved after the section header table: only the last program header,
/// which only the count in `sh_info` reaches, says where the image ends.
#[cfg(test)]
fn core_image_ending_in_the_last_extended_segment() -> Vec<u8> {
    let mut image = core_image(EXTENDED + 1, true);
    let last = ELF_HEADER + PROGRAM_HEADER * usize::try_from(EXTENDED).unwrap();
    let offset = u64::try_from(image.len()).unwrap();
    image[last + 8..last + 16].copy_from_slice(&offset.to_le_bytes());
    image.extend_from_slice(&[0_u8; 16]);
    assert!(section_of(&image) + SECTION_HEADER < image.len());
    image
}

#[test]
fn elf_core_end_reads_up_to_the_last_program_header_sh_info_names() {
    let image = core_image_ending_in_the_last_extended_segment();
    assert_eq!(elf_core_end(&image), Some(image.len()));
    // A count one short, still a valid extended count, ends at the section
    // table instead.
    let mut short = image.clone();
    let section = section_of(&short);
    short[section + 0x2c..section + 0x30].copy_from_slice(&EXTENDED.to_le_bytes());
    assert_eq!(elf_core_end(&short), Some(section + SECTION_HEADER));
}

#[test]
fn elf_core_end_refuses_bad_entry_sizes_counts_and_tables() {
    let image = core_image(EXTENDED, true);
    let section = section_of(&image);
    let set = |at: std::ops::Range<usize>, bytes: &[u8]| {
        let mut changed = image.clone();
        changed[at].copy_from_slice(bytes);
        changed
    };
    let cases: Vec<(&str, Vec<u8>)> = vec![
        (
            "a zero program header size",
            set(0x36..0x38, &0_u16.to_le_bytes()),
        ),
        (
            "a section header size other than 64",
            set(0x3a..0x3c, &40_u16.to_le_bytes()),
        ),
        (
            "a first section header that is not null",
            set(section + 4..section + 8, &1_u32.to_le_bytes()),
        ),
        (
            "PN_XNUM with no section header",
            set(0x3c..0x3e, &0_u16.to_le_bytes()),
        ),
        (
            "an extended count of zero",
            set(section + 0x2c..section + 0x30, &0_u32.to_le_bytes()),
        ),
        (
            "an extended count of three",
            set(section + 0x2c..section + 0x30, &3_u32.to_le_bytes()),
        ),
        (
            "an extended count one below PN_XNUM",
            set(
                section + 0x2c..section + 0x30,
                &(EXTENDED - 1).to_le_bytes(),
            ),
        ),
        (
            "a count whose table runs past the image",
            set(section + 0x2c..section + 0x30, &u32::MAX.to_le_bytes()),
        ),
        (
            "a program header table inside the ELF header",
            set(0x20..0x28, &8_u64.to_le_bytes()),
        ),
        (
            "a section header table inside the ELF header",
            set(0x28..0x30, &8_u64.to_le_bytes()),
        ),
        (
            "a table offset that overflows",
            set(0x20..0x28, &u64::MAX.to_le_bytes()),
        ),
        (
            "a section table offset that overflows",
            set(0x28..0x30, &(u64::MAX - 8).to_le_bytes()),
        ),
    ];
    for (case, image) in cases {
        assert_eq!(elf_core_end(&image), None, "{case}");
    }
}

#[test]
fn elf_core_end_refuses_what_is_not_an_elf_core() {
    let mut image = core_image(1, false);
    image[16] = 2;
    assert_eq!(elf_core_end(&image), None);
    assert_eq!(elf_core_end(b"\x7fELF"), None);
}
