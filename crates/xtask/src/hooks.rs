// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Management of the opt-in Git pre-push hook.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{self, Command, Stdio};

use crate::Error;

#[cfg(test)]
mod tests;

const HOOK_HEADER: &str = "\
#!/bin/sh
# SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
# SPDX-License-Identifier: GPL-3.0-or-later
# Managed by IronAFS cargo xtask
";

/// Installs the pre-push hook in the directory Git reports.
///
/// # Errors
///
/// Returns an error if Git or the filesystem fails, or a foreign hook exists
/// and `force` is false.
pub(super) fn install(force: bool) -> Result<(), Error> {
    let directory = hooks_directory()?;
    let path = directory.join("pre-push");
    if !force && managed_hook_exists(&path)? {
        return Ok(());
    }

    fs::create_dir_all(&directory).map_err(Error::io("create hooks directory"))?;
    let staged = directory.join(format!(".ironafs-pre-push-{}", process::id()));
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&staged)
        .map_err(Error::io("create temporary pre-push hook"))?;
    let written = write_hook(&mut file).map_err(Error::io("write pre-push hook"));
    drop(file);
    let installed = written.and_then(|()| publish_hook(&staged, &path, force));
    if force && installed.is_ok() {
        return Ok(());
    }
    let cleaned = fs::remove_file(staged).map_err(Error::io("remove temporary pre-push hook"));
    installed.and(cleaned)
}

fn write_hook(file: &mut File) -> io::Result<()> {
    file.write_all(HOOK_HEADER.as_bytes())?;
    file.write_all(b"exec cargo xtask check\n")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        file.set_permissions(fs::Permissions::from_mode(0o755))?;
    }
    Ok(())
}

fn publish_hook(staged: &Path, path: &Path, force: bool) -> Result<(), Error> {
    if force {
        return fs::rename(staged, path).map_err(Error::io("replace pre-push hook"));
    }

    // A hard link publishes the complete hook and fails if another hook took the path.
    match fs::hard_link(staged, path) {
        Err(error)
            if error.kind() == io::ErrorKind::AlreadyExists && managed_hook_exists(path)? =>
        {
            Ok(())
        }
        result => result.map_err(Error::io("install pre-push hook")),
    }
}

/// Removes a managed pre-push hook. An absent hook is already uninstalled.
///
/// # Errors
///
/// Returns an error if Git or the filesystem fails, or the hook is foreign.
pub(super) fn uninstall() -> Result<(), Error> {
    let path = hooks_directory()?.join("pre-push");
    if managed_hook_exists(&path)? {
        fs::remove_file(path).map_err(Error::io("remove pre-push hook"))?;
    }
    Ok(())
}

fn managed_hook_exists(path: &Path) -> Result<bool, Error> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() => {
            let mut header = [0; HOOK_HEADER.len()];
            let mut file = File::open(path).map_err(Error::io("open pre-push hook"))?;
            match file.read_exact(&mut header) {
                Ok(()) if is_managed_hook(&header) => return Ok(true),
                Ok(()) => (),
                Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => (),
                Err(error) => return Err(Error::io("read pre-push hook")(error)),
            }
        }
        Ok(_) => (),
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(Error::io("inspect pre-push hook")(error)),
    }
    Err(Error::ForeignHook(path.to_path_buf()))
}

fn is_managed_hook(contents: &[u8]) -> bool {
    contents.starts_with(HOOK_HEADER.as_bytes())
}

fn hooks_directory() -> Result<PathBuf, Error> {
    const COMMAND: &str = "git rev-parse --git-path hooks";
    let output = Command::new("git")
        .args(["rev-parse", "--git-path", "hooks"])
        .stderr(Stdio::inherit())
        .output()
        .map_err(Error::io(COMMAND))?;
    Error::check_status(COMMAND, output.status)?;
    let path = output
        .stdout
        .strip_suffix(b"\n")
        .ok_or(Error::UnexpectedOutput(COMMAND))?;
    #[cfg(windows)]
    let path = path.strip_suffix(b"\r").unwrap_or(path);
    if path.is_empty() || path.contains(&0) {
        return Err(Error::UnexpectedOutput(COMMAND));
    }

    #[cfg(unix)]
    {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;

        Ok(PathBuf::from(OsStr::from_bytes(path)))
    }
    #[cfg(not(unix))]
    {
        std::str::from_utf8(path)
            .map(PathBuf::from)
            .map_err(|_| Error::UnexpectedOutput(COMMAND))
    }
}
