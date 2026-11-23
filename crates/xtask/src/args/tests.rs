// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for command routing and rejected arguments.

use std::ffi::OsString;

use super::{CellCommand, Task, TrailerMode, UsageError, parse};

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
            &["check-fuzz-deny"][..],
            Task::CheckFuzzDeny { self_test: false },
        ),
        (
            &["check-fuzz-deny", "--self-test"][..],
            Task::CheckFuzzDeny { self_test: true },
        ),
        (
            &["check-grants"][..],
            Task::CheckGrants { self_test: false },
        ),
        (
            &["check-grants", "--self-test"][..],
            Task::CheckGrants { self_test: true },
        ),
        (&["check-bans"][..], Task::CheckBans { self_test: false }),
        (
            &["check-bans", "--self-test"][..],
            Task::CheckBans { self_test: true },
        ),
        (&["check-pure"][..], Task::CheckPure { self_test: false }),
        (&["fuzz"][..], Task::Fuzz { self_test: false }),
        (&["fuzz", "--self-test"][..], Task::Fuzz { self_test: true }),
        (
            &["check-pure", "--self-test"][..],
            Task::CheckPure { self_test: true },
        ),
        (&["check-os"][..], Task::CheckOs { self_test: false }),
        (
            &["check-os", "--self-test"][..],
            Task::CheckOs { self_test: true },
        ),
        (&["check-seams"][..], Task::CheckSeams { self_test: false }),
        (
            &["check-seams", "--self-test"][..],
            Task::CheckSeams { self_test: true },
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
            &["check-fuzz-deny", "--all"][..],
            UsageError::UnexpectedArgument("--all".into()),
        ),
        (
            &["check-bans", "--all"][..],
            UsageError::UnexpectedArgument("--all".into()),
        ),
        (
            &["check-pure", "--all"][..],
            UsageError::UnexpectedArgument("--all".into()),
        ),
        (
            &["check-seams", "--all"][..],
            UsageError::UnexpectedArgument("--all".into()),
        ),
    ] {
        assert_eq!(parse(args.iter().map(OsString::from)), Err(error));
    }
}

#[test]
fn parse_routes_check_abi_with_and_without_a_target() {
    for (args, task) in [
        (
            &["check-abi"][..],
            Task::CheckAbi {
                self_test: false,
                target: None,
            },
        ),
        (
            &["check-abi", "--self-test"][..],
            Task::CheckAbi {
                self_test: true,
                target: None,
            },
        ),
        (
            &["check-abi", "--target", "aarch64-unknown-linux-musl"][..],
            Task::CheckAbi {
                self_test: false,
                target: Some("aarch64-unknown-linux-musl".into()),
            },
        ),
        (&["check-wipe"][..], Task::CheckWipe),
    ] {
        assert_eq!(parse(args.iter().map(OsString::from)), Ok(task));
    }
    for (args, error) in [
        (&["check-abi", "--target"][..], UsageError::MissingTarget),
        (
            &["check-abi", "--target", "x", "y"][..],
            UsageError::UnexpectedArgument("y".into()),
        ),
    ] {
        assert_eq!(parse(args.iter().map(OsString::from)), Err(error));
    }
}

#[test]
fn parse_cell_pin() {
    assert_eq!(
        parse(["cell", "pin"].into_iter().map(OsString::from)),
        Ok(Task::Cell(CellCommand::Pin))
    );
}

#[test]
fn parse_cell_unknown_subcommand_reports_usage() {
    for (args, error) in [
        (&["cell"][..], UsageError::MissingCellSubcommand),
        (
            &["cell", "start"][..],
            UsageError::UnknownCellSubcommand("start".into()),
        ),
        (
            &["cell", "pin", "extra"][..],
            UsageError::UnexpectedArgument("extra".into()),
        ),
    ] {
        assert_eq!(parse(args.iter().map(OsString::from)), Err(error));
    }
}

#[test]
fn usage_names_the_cell_command() {
    let usage = UsageError::MissingCellSubcommand.to_string();

    assert_eq!(usage.lines().next(), Some("missing cell subcommand"));
    assert!(usage.ends_with(" | cell pin"));
}
