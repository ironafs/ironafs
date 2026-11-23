// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The pinned compiler, resolved once for every query and build of
//! `check-os`'s target checks (ADR 0005, point 5.3): the target list and
//! cfg sets, the gate's standalone compilation and the Cargo builds.
//!
//! The toolchain file's `channel` is the pin. The `rustc` and `cargo` the
//! environment selects must both report that release (`-vV`), or the check
//! fails before it reads a target: an override (`RUSTUP_TOOLCHAIN`, a
//! `+toolchain`, a directory override) would otherwise check another
//! compiler's targets. The compiler is then named by its path in that
//! sysroot, and Cargo is given it as `RUSTC`.

use std::path::{Path, PathBuf};

use crate::pure::CLEARED_ENVIRONMENT;
use crate::{Command, Error};

#[cfg(test)]
mod tests;

/// The pinned compiler, checked against the toolchain file.
#[derive(Debug)]
pub(crate) struct Pinned {
    /// The compiler, by its path in the pinned sysroot.
    pub(crate) rustc: PathBuf,
}

/// The `channel` of a `rust-toolchain.toml` text.
pub(crate) fn channel(text: &str) -> Option<&str> {
    text.lines()
        .map(|line| line.split('#').next().unwrap_or_default().trim())
        .find_map(|line| {
            line.strip_prefix("channel")
                .map(str::trim_start)
                .and_then(|rest| rest.strip_prefix('='))
        })
        .map(|value| value.trim().trim_matches('"'))
}

/// The `release:` line of a tool's `-vV` output.
pub(crate) fn release(verbose: &str) -> Option<&str> {
    verbose
        .lines()
        .find_map(|line| line.strip_prefix("release:"))
        .map(str::trim)
}

/// Why `tool`, reporting `found`, is not the pinned `pin`, if it is not.
pub(crate) fn mismatch(tool: &str, found: Option<&str>, pin: &str) -> Option<String> {
    match found {
        Some(found) if found == pin => None,
        Some(found) => Some(format!(
            "{tool} reports release {found}, not the toolchain file's pin {pin}: the target \
             checks run only on the pinned compiler"
        )),
        None => Some(format!(
            "{tool} -vV reports no release, so it cannot be the pin {pin}"
        )),
    }
}

/// Runs `program -vV` from `root` and returns its release line.
fn release_of(root: &Path, program: &Path) -> Result<Option<String>, Error> {
    let output = Command::new(program)
        .current_dir(root)
        .arg("-vV")
        .output()
        .map_err(Error::io("run a tool's -vV"))?;
    Error::check_status("-vV", output.status)?;
    Ok(release(&String::from_utf8_lossy(&output.stdout)).map(str::to_owned))
}

/// Resolves the pinned compiler from `root`, or says why the selected
/// tools are not the pin.
///
/// # Errors
///
/// Returns an error if the toolchain file cannot be read or a tool cannot
/// be run.
pub(crate) fn resolve(root: &Path) -> Result<Result<Pinned, String>, Error> {
    let path = root.join("rust-toolchain.toml");
    let text =
        std::fs::read_to_string(&path).map_err(Error::path_io("read toolchain file", &path))?;
    let Some(pin) = channel(&text) else {
        return Ok(Err("rust-toolchain.toml names no channel".to_owned()));
    };
    for tool in ["rustc", "cargo"] {
        if let Some(problem) = mismatch(tool, release_of(root, Path::new(tool))?.as_deref(), pin) {
            return Ok(Err(problem));
        }
    }
    let rustc = crate::pure::toolchain_sysroot(root)?
        .join("bin")
        .join(format!("rustc{}", std::env::consts::EXE_SUFFIX));
    if let Some(problem) = mismatch(
        &rustc.display().to_string(),
        release_of(root, &rustc)?.as_deref(),
        pin,
    ) {
        return Ok(Err(problem));
    }
    Ok(Ok(Pinned { rustc }))
}

/// Sets `command`, a Cargo run of `check-os`, to the pinned compiler and
/// no compiler wrapper, whatever the environment or Cargo's configuration
/// selects: `RUSTC` names the pinned compiler, and an empty
/// `RUSTC_WRAPPER` and `RUSTC_WORKSPACE_WRAPPER` override a configured
/// wrapper. The other variables that change what the compiler is given
/// are removed.
pub(crate) fn pin_cargo(command: &mut Command, pinned: &Pinned) {
    for name in CLEARED_ENVIRONMENT {
        command.env_remove(name);
    }
    command
        .env("RUSTC", &pinned.rustc)
        .env("RUSTC_WRAPPER", "")
        .env("RUSTC_WORKSPACE_WRAPPER", "");
}

/// Sets `command`, a Cargo build of `check-os`, to the pinned compiler with
/// no wrapper (`pin_cargo`), building into `build_dir` for both Cargo's
/// target directory and its build directory (`bans::isolated_build`), so
/// no configured `build.build-dir` shares artifacts with other builds.
pub(crate) fn pinned_build(command: &mut Command, pinned: &Pinned, build_dir: &Path) {
    pin_cargo(command, pinned);
    crate::bans::isolated_build(command, build_dir);
}
