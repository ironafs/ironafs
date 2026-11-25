// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Command-line grammar for repository automation.

use std::ffi::{OsStr, OsString};
use std::fmt;
use std::path::PathBuf;

use crate::abi::{Mode, legacy_target};

#[cfg(test)]
mod tests;

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Task {
    Check,
    CheckBans {
        self_test: bool,
    },
    CheckDeps {
        self_test: bool,
    },
    CheckFuzzDeny {
        self_test: bool,
    },
    CheckGrants {
        self_test: bool,
    },
    CheckPure {
        self_test: bool,
    },
    CheckSeams {
        self_test: bool,
    },
    CheckOs {
        self_test: bool,
    },
    CheckAbi {
        self_test: bool,
        target: Option<String>,
        mode: Mode,
    },
    CheckWipe {
        self_test: bool,
    },
    Fuzz {
        self_test: bool,
    },
    InstallHook {
        force: bool,
    },
    UninstallHook,
    Trailers(TrailerMode),
    Cell(CellCommand),
}

/// A `cell` subcommand.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CellCommand {
    Pin,
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
    MissingTarget,
    MissingCellSubcommand,
    UnknownCommand(OsString),
    UnknownCellSubcommand(OsString),
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
            Self::MissingTarget => write!(formatter, "missing target triple"),
            Self::UnknownCommand(command) => {
                write!(formatter, "unknown command {}", command.display())
            }
            Self::UnknownHookSubcommand(command) => {
                write!(formatter, "unknown hooks subcommand {}", command.display())
            }
            Self::UnknownTrailerMode(mode) => {
                write!(formatter, "unknown trailers mode {}", mode.display())
            }
            Self::MissingCellSubcommand => write!(formatter, "missing cell subcommand"),
            Self::UnknownCellSubcommand(command) => {
                write!(formatter, "unknown cell subcommand {}", command.display())
            }
            Self::UnexpectedArgument(argument) => {
                write!(formatter, "unexpected argument {}", argument.display())
            }
        }?;
        write!(
            formatter,
            "\nusage: cargo xtask check | check-bans [--self-test] | check-deps [--self-test] | check-fuzz-deny [--self-test] | check-grants [--self-test] | check-pure [--self-test] | check-seams [--self-test] | check-os [--self-test] | check-abi [--self-test | --target <triple> [--legacy-glibc-2-28]] | check-wipe [--self-test] | fuzz [--self-test] | hooks install [--force] | hooks uninstall | trailers pr | trailers commit <file> | trailers main [file] | cell pin"
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
        Some("check-bans") => Task::CheckBans {
            self_test: parse_self_test(&mut arguments)?,
        },
        Some("check-deps") => Task::CheckDeps {
            self_test: parse_self_test(&mut arguments)?,
        },
        Some("check-fuzz-deny") => Task::CheckFuzzDeny {
            self_test: parse_self_test(&mut arguments)?,
        },
        Some("check-grants") => Task::CheckGrants {
            self_test: parse_self_test(&mut arguments)?,
        },
        Some("check-pure") => Task::CheckPure {
            self_test: parse_self_test(&mut arguments)?,
        },
        Some("check-os") => Task::CheckOs {
            self_test: parse_self_test(&mut arguments)?,
        },
        Some("check-abi") => parse_check_abi(&mut arguments)?,
        Some("check-seams") => Task::CheckSeams {
            self_test: parse_self_test(&mut arguments)?,
        },
        Some("check-wipe") => Task::CheckWipe {
            self_test: parse_self_test(&mut arguments)?,
        },
        Some("fuzz") => Task::Fuzz {
            self_test: parse_self_test(&mut arguments)?,
        },
        Some("hooks") => parse_hooks(&mut arguments)?,
        Some("trailers") => Task::Trailers(parse_trailers(&mut arguments)?),
        Some("cell") => Task::Cell(parse_cell(&mut arguments)?),
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

fn parse_check_abi(arguments: &mut impl Iterator<Item = OsString>) -> Result<Task, UsageError> {
    let legacy = OsStr::new("--legacy-glibc-2-28");
    match arguments.next() {
        None => Ok(Task::CheckAbi {
            self_test: false,
            target: None,
            mode: Mode::Complete,
        }),
        Some(argument) if argument == OsStr::new("--self-test") => Ok(Task::CheckAbi {
            self_test: true,
            target: None,
            mode: Mode::Complete,
        }),
        Some(argument) if argument == OsStr::new("--target") || argument == legacy => {
            let legacy_first = argument == legacy;
            if legacy_first {
                let option = arguments.next().ok_or(UsageError::MissingTarget)?;
                if option != OsStr::new("--target") {
                    return Err(UsageError::UnexpectedArgument(option));
                }
            }
            let target = arguments
                .next()
                .ok_or(UsageError::MissingTarget)?
                .into_string()
                .map_err(UsageError::UnexpectedArgument)?;
            let legacy_selected = if legacy_first {
                true
            } else {
                match arguments.next() {
                    Some(option) if option == legacy => true,
                    Some(option) => return Err(UsageError::UnexpectedArgument(option)),
                    None => false,
                }
            };
            let mode = if legacy_selected {
                if !legacy_target(&target) {
                    return Err(UsageError::UnexpectedArgument(target.into()));
                }
                Mode::LegacyGlibc228
            } else {
                Mode::Complete
            };
            Ok(Task::CheckAbi {
                self_test: false,
                target: Some(target),
                mode,
            })
        }
        Some(argument) => Err(UsageError::UnexpectedArgument(argument)),
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

fn parse_cell(arguments: &mut impl Iterator<Item = OsString>) -> Result<CellCommand, UsageError> {
    let command = arguments.next().ok_or(UsageError::MissingCellSubcommand)?;
    match command.to_str() {
        Some("pin") => Ok(CellCommand::Pin),
        _ => Err(UsageError::UnknownCellSubcommand(command)),
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
