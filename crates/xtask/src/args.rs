// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Command-line grammar for repository automation.

use std::ffi::{OsStr, OsString};
use std::fmt;

#[cfg(test)]
mod tests;

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Task {
    Check,
    InstallHook { force: bool },
    UninstallHook,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum UsageError {
    MissingCommand,
    MissingHookSubcommand,
    UnknownCommand(OsString),
    UnknownHookSubcommand(OsString),
    UnexpectedArgument(OsString),
}

impl fmt::Display for UsageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingCommand => write!(formatter, "missing command"),
            Self::MissingHookSubcommand => write!(formatter, "missing hooks subcommand"),
            Self::UnknownCommand(command) => {
                write!(formatter, "unknown command {}", command.display())
            }
            Self::UnknownHookSubcommand(command) => {
                write!(formatter, "unknown hooks subcommand {}", command.display())
            }
            Self::UnexpectedArgument(argument) => {
                write!(formatter, "unexpected argument {}", argument.display())
            }
        }?;
        write!(
            formatter,
            "\nusage: cargo xtask check | hooks install [--force] | hooks uninstall"
        )
    }
}

/// Parses one command, rejecting options outside `hooks install`.
///
/// # Errors
///
/// Returns a usage error if the arguments do not specify a supported command.
pub(super) fn parse(mut arguments: impl Iterator<Item = OsString>) -> Result<Task, UsageError> {
    let command = arguments.next().ok_or(UsageError::MissingCommand)?;
    let task = match command.to_str() {
        Some("check") => Task::Check,
        Some("hooks") => parse_hooks(&mut arguments)?,
        _ => return Err(UsageError::UnknownCommand(command)),
    };
    if let Some(argument) = arguments.next() {
        return Err(UsageError::UnexpectedArgument(argument));
    }
    Ok(task)
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
