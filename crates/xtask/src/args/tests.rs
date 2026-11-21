// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for command routing and rejected arguments.

use std::ffi::OsString;

use super::{Task, TrailerMode, UsageError, parse};

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

#[test]
fn parse_routes_each_trailer_mode_and_message_path() {
    for (args, mode) in [
        (&["trailers", "pr"][..], TrailerMode::Pr),
        (
            &["trailers", "commit", "commit message"][..],
            TrailerMode::Commit("commit message".into()),
        ),
        (&["trailers", "main"][..], TrailerMode::Main(None)),
        (
            &["trailers", "main", "commit message"][..],
            TrailerMode::Main(Some("commit message".into())),
        ),
    ] {
        assert_eq!(
            parse(args.iter().map(OsString::from)),
            Ok(Task::Trailers(mode))
        );
    }
}

#[test]
fn parse_rejects_missing_unknown_and_extra_trailer_arguments() {
    for (args, error) in [
        (&["trailers"][..], UsageError::MissingTrailerMode),
        (
            &["trailers", "commit"][..],
            UsageError::MissingCommitMessagePath,
        ),
        (
            &["trailers", "test"][..],
            UsageError::UnknownTrailerMode("test".into()),
        ),
        (
            &["trailers", "pr", "file"][..],
            UsageError::UnexpectedArgument("file".into()),
        ),
        (
            &["trailers", "main", "file", "extra"][..],
            UsageError::UnexpectedArgument("extra".into()),
        ),
        (
            &["trailers", "commit", "file", "extra"][..],
            UsageError::UnexpectedArgument("extra".into()),
        ),
    ] {
        assert_eq!(parse(args.iter().map(OsString::from)), Err(error));
    }
}

#[test]
fn parse_routes_rule_checks_with_and_without_self_test() {
    for (args, task) in [
        (&["check-deps"][..], Task::CheckDeps { self_test: false }),
        (
            &["check-deps", "--self-test"][..],
            Task::CheckDeps { self_test: true },
        ),
        (
            &["check-grants"][..],
            Task::CheckGrants { self_test: false },
        ),
        (
            &["check-grants", "--self-test"][..],
            Task::CheckGrants { self_test: true },
        ),
        (&["check-pure"][..], Task::CheckPure { self_test: false }),
        (
            &["check-pure", "--self-test"][..],
            Task::CheckPure { self_test: true },
        ),
        (
            &["check-retries"][..],
            Task::CheckRetries { self_test: false },
        ),
        (
            &["check-retries", "--self-test"][..],
            Task::CheckRetries { self_test: true },
        ),
    ] {
        assert_eq!(parse(args.iter().map(OsString::from)), Ok(task));
    }
    for (args, error) in [
        (
            &["check-deps", "--force"][..],
            UsageError::UnexpectedArgument("--force".into()),
        ),
        (
            &["check-grants", "--self-test", "extra"][..],
            UsageError::UnexpectedArgument("extra".into()),
        ),
        (
            &["check-pure", "--all"][..],
            UsageError::UnexpectedArgument("--all".into()),
        ),
    ] {
        assert_eq!(parse(args.iter().map(OsString::from)), Err(error));
    }
}
