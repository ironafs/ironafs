// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Filesystem steps several commands share, each taking the caller's
//! mapping from an I/O error to its own error type.

use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::process;
use std::sync::atomic::{AtomicU64, Ordering};

#[cfg(test)]
mod tests;

/// Removes `dir` and everything in it; a missing directory is already
/// removed.
///
/// # Errors
///
/// Returns `error` applied to the failure if the directory exists and
/// cannot be removed.
pub(crate) fn remove_dir<E>(dir: &Path, error: impl FnOnce(io::Error) -> E) -> Result<(), E> {
    match fs::remove_dir_all(dir) {
        Err(failure) if failure.kind() != io::ErrorKind::NotFound => Err(error(failure)),
        _ => Ok(()),
    }
}

/// Creates a new file in `directory` under a name no other process or call
/// uses, `.ironafs-<tag>-<pid>-<n>`, fills it with `write`, and hands its
/// path to `publish`, which renames or links it into place. The staged file
/// is removed afterwards unless `publish` moved it.
///
/// # Errors
///
/// Returns `io_error` applied to the operation, path and failure of a
/// staging step, and `publish`'s error; a failure to remove the staged
/// file is reported after `publish`'s own.
pub(crate) fn write_staged<E>(
    directory: &Path,
    tag: &str,
    write: impl FnOnce(&mut File) -> io::Result<()>,
    publish: impl FnOnce(&Path) -> Result<(), E>,
    io_error: impl Fn(&'static str, &Path, io::Error) -> E,
) -> Result<(), E> {
    let staged = staged_path(directory, tag);
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&staged)
        .map_err(|error| io_error("create temporary file", &staged, error))?;
    let written =
        write(&mut file).map_err(|error| io_error("write temporary file", &staged, error));
    drop(file);
    let published = written.and_then(|()| publish(&staged));
    let cleaned = match fs::remove_file(&staged) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => {
            Err(io_error("remove temporary file", &staged, error))
        }
        _ => Ok(()),
    };
    published.and(cleaned)
}

/// The next staging path for `tag` in `directory`.
fn staged_path(directory: &Path, tag: &str) -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    directory.join(format!(".ironafs-{tag}-{}-{n}", process::id()))
}
