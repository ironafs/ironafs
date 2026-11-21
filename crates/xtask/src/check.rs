// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Runs local checks in order and reports missing optional tools.

use std::io::{self, Write};
use std::process::{ExitStatus, Stdio};

use crate::{Command, Error};

#[cfg(test)]
mod tests;

/// Runs the local checks, printing a skip for each absent optional tool.
///
/// # Errors
///
/// Returns the first check failure, including failure to start a required tool.
pub(super) fn run(output: &mut impl Write) -> Result<(), Error> {
    run_command(
        Command::new("cargo").args(["fmt", "--all", "--check"]),
        "cargo fmt --all --check",
    )?;
    run_command(
        Command::new("cargo")
            .args(["clippy", "--all-targets", "--workspace"])
            .env("CARGO_BUILD_WARNINGS", "deny"),
        "cargo clippy --all-targets --workspace",
    )?;
    run_command(
        Command::new("cargo").args([
            "nextest",
            "run",
            "--workspace",
            "--retries",
            "0",
            "--no-tests",
            "fail",
        ]),
        "cargo nextest run --workspace --retries 0 --no-tests fail",
    )?;
    run_command(
        Command::new("cargo").args(["test", "--doc", "--workspace"]),
        "cargo test --doc --workspace",
    )?;
    for self_test in [false, true] {
        crate::bans::run(self_test, output)?;
        crate::deps::run(self_test, output)?;
        crate::grants::run(self_test, output)?;
        crate::pure::run(self_test, output)?;
        crate::os::run(self_test, output)?;
    }
    // check-abi proves the host's own foreign declarations against its C
    // headers; on a host with none it has nothing to prove (and would fail
    // as a run that proves nothing), so the local aggregate says so. CI
    // runs it, unconditionally, on each Linux target that has rows.
    let (applies, host) = crate::abi::applies_here()?;
    if applies {
        crate::abi::run(false, None, output)?;
        crate::abi::run(true, None, output)?;
    } else {
        writeln!(
            output,
            "skipped check-abi: no row with a foreign item is implemented on {host}"
        )
        .map_err(Error::io("report skipped check"))?;
    }
    crate::wipe::run(output)?;

    check_reuse_result(
        Command::new("reuse").arg("lint").status(),
        reuse_in_path,
        output,
    )?;
    if cargo_has_deny()? {
        run_command(
            Command::new("cargo").args(["deny", "check"]),
            "cargo deny check",
        )
    } else {
        report_skipped("cargo-deny", output)
    }
}

fn run_command(command: &mut Command, name: &'static str) -> Result<(), Error> {
    let status = command.status().map_err(Error::io(name))?;
    Error::check_status(name, status)
}

fn check_reuse_result(
    result: io::Result<ExitStatus>,
    reuse_in_path: impl FnOnce() -> bool,
    output: &mut impl Write,
) -> Result<(), Error> {
    match result {
        Ok(status) => Error::check_status("reuse lint", status),
        Err(error) if error.kind() == io::ErrorKind::NotFound && !reuse_in_path() => {
            report_skipped("reuse", output)
        }
        Err(error) => Err(Error::io("run reuse lint")(error)),
    }
}

fn reuse_in_path() -> bool {
    // The OS also reports NotFound for an installed script with a missing interpreter.
    let executable = if cfg!(windows) { "reuse.exe" } else { "reuse" };
    std::env::var_os("PATH").is_some_and(|path| {
        std::env::split_paths(&path).any(|dir| dir.join(executable).symlink_metadata().is_ok())
    })
}

fn cargo_has_deny() -> Result<bool, Error> {
    let output = Command::new("cargo")
        .arg("--list")
        .stderr(Stdio::inherit())
        .output()
        .map_err(Error::io("run cargo --list"))?;
    Error::check_status("cargo --list", output.status)?;
    lists_deny(&output.stdout)
}

fn lists_deny(output: &[u8]) -> Result<bool, Error> {
    let listing = String::from_utf8_lossy(output);
    let mut lines = listing.lines();
    if lines.next() != Some("Installed Commands:") {
        return Err(Error::UnexpectedOutput("cargo --list"));
    }
    let commands = lines.filter_map(|line| line.split_whitespace().next());
    // Cargo puts the built-in commands in the same list as external commands.
    let (has_check, has_deny) = commands.fold((false, false), |(check, deny), name| {
        (check || name == "check", deny || name == "deny")
    });
    if !has_check {
        return Err(Error::UnexpectedOutput("cargo --list"));
    }
    Ok(has_deny)
}

fn report_skipped(tool: &str, output: &mut impl Write) -> Result<(), Error> {
    writeln!(output, "skipped {tool} (not installed)").map_err(Error::io("report skipped tool"))
}
