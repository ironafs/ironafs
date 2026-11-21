// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Command-line grammar for repository automation.

use std::ffi::{OsStr, OsString};
use std::fmt;
use std::path::PathBuf;

#[cfg(test)]
mod tests;

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Task {
    Check,
    CheckDeps { self_test: bool },
    CheckGrants { self_test: bool },
    CheckPure { self_test: bool },
    Inventory(InventoryMode),
    InstallHook { force: bool },
    UninstallHook,
    Trailers(TrailerMode),
}

/// What `cargo xtask inventory` does.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum InventoryMode {
    /// Runs the tests and checks the inventory against the run and against
    /// the base revision's inventory (by default, the merge base with
    /// `origin/main`).
    Check {
        base: Option<String>,
    },
    SelfTest,
    /// Adds a line for each new test the host's run lists.
    Update,
}

/// Selects the trailer rules and where to read the message.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum TrailerMode {
    Pr,
    Commit(PathBuf),
    Main(Option<PathBuf>),
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum UsageError {
    MissingCommand,
    MissingHookSubcommand,
    MissingTrailerMode,
    MissingCommitMessagePath,
    MissingBaseRevision,
    UnknownCommand(OsString),
    UnknownHookSubcommand(OsString),
    UnknownTrailerMode(OsString),
    UnexpectedArgument(OsString),
}

impl fmt::Display for UsageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingCommand => write!(formatter, "missing command"),
            Self::MissingHookSubcommand => write!(formatter, "missing hooks subcommand"),
            Self::MissingTrailerMode => write!(formatter, "missing trailers mode"),
            Self::MissingCommitMessagePath => write!(formatter, "missing commit message file path"),
            Self::MissingBaseRevision => write!(formatter, "missing base revision after --base"),
            Self::UnknownCommand(command) => {
                write!(formatter, "unknown command {}", command.display())
            }
            Self::UnknownHookSubcommand(command) => {
                write!(formatter, "unknown hooks subcommand {}", command.display())
            }
            Self::UnknownTrailerMode(mode) => {
                write!(formatter, "unknown trailers mode {}", mode.display())
            }
            Self::UnexpectedArgument(argument) => {
                write!(formatter, "unexpected argument {}", argument.display())
            }
        }?;
        write!(
            formatter,
            "\nusage: cargo xtask check | check-deps [--self-test] | check-grants [--self-test] | check-pure [--self-test] | inventory [--base <rev> | --self-test | update] | hooks install [--force] | hooks uninstall | trailers pr | trailers commit <file> | trailers main [file]"
        )
    }
}

/// Parses one command and rejects extra arguments.
///
/// # Errors
///
/// Returns a usage error if the arguments do not specify a supported command.
pub(super) fn parse(mut arguments: impl Iterator<Item = OsString>) -> Result<Task, UsageError> {
    let command = arguments.next().ok_or(UsageError::MissingCommand)?;
    let task = match command.to_str() {
        Some("check") => Task::Check,
        Some("check-deps") => Task::CheckDeps {
            self_test: parse_self_test(&mut arguments)?,
        },
        Some("check-grants") => Task::CheckGrants {
            self_test: parse_self_test(&mut arguments)?,
        },
        Some("check-pure") => Task::CheckPure {
            self_test: parse_self_test(&mut arguments)?,
        },
        Some("inventory") => Task::Inventory(parse_inventory(&mut arguments)?),
        Some("hooks") => parse_hooks(&mut arguments)?,
        Some("trailers") => Task::Trailers(parse_trailers(&mut arguments)?),
        _ => return Err(UsageError::UnknownCommand(command)),
    };
    if let Some(argument) = arguments.next() {
        return Err(UsageError::UnexpectedArgument(argument));
    }
    Ok(task)
}

fn parse_self_test(arguments: &mut impl Iterator<Item = OsString>) -> Result<bool, UsageError> {
    match arguments.next() {
        Some(argument) if argument == OsStr::new("--self-test") => Ok(true),
        Some(argument) => Err(UsageError::UnexpectedArgument(argument)),
        None => Ok(false),
    }
}

fn parse_inventory(
    arguments: &mut impl Iterator<Item = OsString>,
) -> Result<InventoryMode, UsageError> {
    let Some(argument) = arguments.next() else {
        return Ok(InventoryMode::Check { base: None });
    };
    match argument.to_str() {
        Some("--self-test") => Ok(InventoryMode::SelfTest),
        Some("update") => Ok(InventoryMode::Update),
        Some("--base") => {
            let base = arguments.next().ok_or(UsageError::MissingBaseRevision)?;
            base.into_string()
                .map(|base| InventoryMode::Check { base: Some(base) })
                .map_err(UsageError::UnexpectedArgument)
        }
        _ => Err(UsageError::UnexpectedArgument(argument)),
    }
}

fn parse_trailers(
    arguments: &mut impl Iterator<Item = OsString>,
) -> Result<TrailerMode, UsageError> {
    let mode = arguments.next().ok_or(UsageError::MissingTrailerMode)?;
    match mode.to_str() {
        Some("pr") => Ok(TrailerMode::Pr),
        Some("commit") => arguments
            .next()
            .map(|path| TrailerMode::Commit(path.into()))
            .ok_or(UsageError::MissingCommitMessagePath),
        Some("main") => Ok(TrailerMode::Main(arguments.next().map(PathBuf::from))),
        _ => Err(UsageError::UnknownTrailerMode(mode)),
    }
}

fn parse_hooks(arguments: &mut impl Iterator<Item = OsString>) -> Result<Task, UsageError> {
    let command = arguments.next().ok_or(UsageError::MissingHookSubcommand)?;
    match command.to_str() {
        Some("install") => match arguments.next() {
            Some(argument) if argument == OsStr::new("--force") => {
                Ok(Task::InstallHook { force: true })
            }
            Some(argument) => Err(UsageError::UnexpectedArgument(argument)),
            None => Ok(Task::InstallHook { force: false }),
        },
        Some("uninstall") => Ok(Task::UninstallHook),
        _ => Err(UsageError::UnknownHookSubcommand(command)),
    }
}
