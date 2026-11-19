// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Management of the opt-in Git hooks.

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

struct Hook {
    name: &'static str,
    command: &'static str,
}

const HOOKS: [Hook; 2] = [
    Hook {
        name: "pre-push",
        command: "exec cargo xtask check\n",
    },
    Hook {
        name: "commit-msg",
        command: "exec cargo xtask trailers commit \"$1\"\n",
    },
];

const RESERVE_BACKUP: &str =
    "reserve hook backup (inspect the backup and remove it if no longer needed)";

/// Installs the pre-push and commit-msg hooks in the directory Git reports.
///
/// # Errors
///
/// Returns an error if Git, filesystem access or backup reporting fails.
/// Replacing a foreign hook requires `force` and an unused backup path.
pub(super) fn install(force: bool, output: &mut impl Write) -> Result<(), Error> {
    install_in(&hooks_directory()?, force, output)
}

fn install_in(directory: &Path, force: bool, output: &mut impl Write) -> Result<(), Error> {
    let mut pending = Vec::new();
    // Check both destinations before changing either hook.
    for hook in HOOKS {
        let path = directory.join(hook.name);
        if needs_install(&path, force)? {
            pending.push((hook, path));
        }
    }
    if !pending.is_empty() {
        fs::create_dir_all(directory).map_err(Error::io("create hooks directory"))?;
        for (hook, path) in pending {
            install_hook(&hook, directory, &path, force, output)?;
        }
    }
    Ok(())
}

fn needs_install(path: &Path, force: bool) -> Result<bool, Error> {
    match managed_hook_exists(path) {
        Ok(managed) => Ok(force || !managed),
        Err(Error::ForeignHook(_)) if force => {
            let backup = path.with_extension("xtask-backup");
            let error = match fs::symlink_metadata(&backup) {
                Ok(_) => io::ErrorKind::AlreadyExists.into(),
                Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(true),
                Err(error) => error,
            };
            Err(Error::hook_io(RESERVE_BACKUP, &backup)(error))
        }
        Err(error) => Err(error),
    }
}

fn install_hook(
    hook: &Hook,
    directory: &Path,
    path: &Path,
    force: bool,
    output: &mut impl Write,
) -> Result<(), Error> {
    let staged = directory.join(format!(".ironafs-{}-{}", hook.name, process::id()));
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&staged)
        .map_err(Error::hook_io("create temporary hook", &staged))?;
    let written =
        write_hook(&mut file, hook.command).map_err(Error::hook_io("write hook", &staged));
    drop(file);
    let installed = written.and_then(|()| publish_hook(&staged, path, force, output));
    if force && installed.is_ok() {
        return Ok(());
    }
    let cleaned =
        fs::remove_file(&staged).map_err(Error::hook_io("remove temporary hook", &staged));
    installed.and(cleaned)
}

fn write_hook(file: &mut File, command: &str) -> io::Result<()> {
    file.write_all(HOOK_HEADER.as_bytes())?;
    file.write_all(command.as_bytes())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        file.set_permissions(fs::Permissions::from_mode(0o755))?;
    }
    Ok(())
}

fn publish_hook(
    staged: &Path,
    path: &Path,
    force: bool,
    output: &mut impl Write,
) -> Result<(), Error> {
    if force {
        let backup = backup_foreign_hook(path)?;
        let replaced = fs::rename(staged, path).map_err(Error::hook_io("replace hook", path));
        if let Some(backup) = backup {
            let reported = writeln!(output, "moved existing hook to {}", backup.display())
                .map_err(Error::io("report hook backup"));
            return replaced.and(reported);
        }
        return replaced;
    }

    // A hard link publishes the complete hook and fails if another hook took the path.
    match fs::hard_link(staged, path) {
        Err(error)
            if error.kind() == io::ErrorKind::AlreadyExists && managed_hook_exists(path)? =>
        {
            Ok(())
        }
        result => result.map_err(Error::hook_io("install hook", path)),
    }
}

/// Moves a foreign hook to its backup path, which must be unused.
fn backup_foreign_hook(path: &Path) -> Result<Option<PathBuf>, Error> {
    match managed_hook_exists(path) {
        Ok(_) => return Ok(None),
        Err(Error::ForeignHook(_)) => (),
        Err(error) => return Err(error),
    }

    let backup = path.with_extension("xtask-backup");
    // Reserve the name atomically so rename can only replace our own file.
    let reservation = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&backup)
        .map_err(Error::hook_io(RESERVE_BACKUP, &backup))?;
    drop(reservation);
    if let Err(error) = fs::rename(path, &backup) {
        let _ = fs::remove_file(&backup);
        return Err(Error::hook_io("move hook to backup", &backup)(error));
    }
    Ok(Some(backup))
}

/// Removes the managed hooks. An absent hook is already uninstalled.
///
/// # Errors
///
/// Returns an error if Git or the filesystem fails, or the hook is foreign.
pub(super) fn uninstall() -> Result<(), Error> {
    uninstall_in(&hooks_directory()?)
}

fn uninstall_in(directory: &Path) -> Result<(), Error> {
    let paths = HOOKS.map(|hook| directory.join(hook.name));
    for path in &paths {
        managed_hook_exists(path)?;
    }
    for path in paths {
        if managed_hook_exists(&path)? {
            fs::remove_file(&path).map_err(Error::hook_io("remove hook", &path))?;
        }
    }
    Ok(())
}

fn managed_hook_exists(path: &Path) -> Result<bool, Error> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() => {
            let mut header = [0; HOOK_HEADER.len()];
            let mut file = File::open(path).map_err(Error::hook_io("open hook", path))?;
            match file.read_exact(&mut header) {
                Ok(()) if is_managed_hook(&header) => return Ok(true),
                Ok(()) => (),
                Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => (),
                Err(error) => return Err(Error::hook_io("read hook", path)(error)),
            }
        }
        Ok(_) => (),
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(Error::hook_io("inspect hook", path)(error)),
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
