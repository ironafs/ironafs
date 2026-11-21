// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The supervisor's own cases, against a program every test machine has:
//! the test binary itself, asked to list its tests.

use std::time::Duration;

use super::{Ended, run};

#[test]
fn a_child_past_the_limit_is_killed_and_reported() {
    let program = std::env::current_exe().unwrap();
    let args = vec!["--list".to_owned()];
    let supervised = run(&program, &args, b"", Duration::ZERO).unwrap();
    assert_eq!(supervised.ended, Ended::OverLimit);
}

#[test]
fn an_exit_status_the_child_never_gives_is_unknown() {
    let program = std::env::current_exe().unwrap();
    let args = vec!["--no-such-option".to_owned()];
    let supervised = run(&program, &args, b"", Duration::from_secs(30)).unwrap();
    assert!(
        matches!(supervised.ended, Ended::Unknown(Some(code)) if code != 0),
        "{:?}",
        supervised.ended
    );
}
