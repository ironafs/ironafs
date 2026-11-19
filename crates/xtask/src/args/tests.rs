// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for command routing and rejected arguments.

use std::ffi::OsString;

use super::{Task, UsageError, parse};

#[test]
fn parse_rejects_unknown_command() {
    assert_eq!(
        parse(["unknown"].into_iter().map(OsString::from)),
        Err(UsageError::UnknownCommand("unknown".into()))
    );
}

#[test]
fn parse_requires_hooks_subcommand() {
    assert_eq!(
        parse(["hooks"].into_iter().map(OsString::from)),
        Err(UsageError::MissingHookSubcommand)
    );
}

#[test]
fn parse_accepts_force_only_for_install() {
    assert_eq!(
        parse(
            ["hooks", "install", "--force"]
                .into_iter()
                .map(OsString::from)
        ),
        Ok(Task::InstallHook { force: true })
    );
    assert_eq!(
        parse(
            ["hooks", "uninstall", "--force"]
                .into_iter()
                .map(OsString::from)
        ),
        Err(UsageError::UnexpectedArgument("--force".into()))
    );
}
