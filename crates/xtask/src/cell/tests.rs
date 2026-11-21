// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for combining a command's result with its teardown's.

use super::{Failure, finish};

fn step_failure(name: &'static str) -> Failure {
    Failure::Step {
        name,
        code: Some(1),
        detail: String::new(),
    }
}

#[test]
fn finish_both_ok_returns_the_value() {
    assert_eq!(finish(Ok(7), Ok(())), Ok(7));
}

#[test]
fn finish_command_failure_is_returned_after_clean_teardown() {
    assert_eq!(
        finish::<()>(Err(step_failure("apt-get update")), Ok(())),
        Err(step_failure("apt-get update"))
    );
}

#[test]
fn finish_teardown_failure_is_reported_after_success() {
    assert_eq!(
        finish(Ok(7), Err(step_failure("remove container"))),
        Err(step_failure("remove container"))
    );
}

#[test]
fn finish_reports_both_failures() {
    assert_eq!(
        finish::<()>(
            Err(step_failure("apt-get update")),
            Err(step_failure("remove container"))
        ),
        Err(Failure::AndTeardown {
            failure: Box::new(step_failure("apt-get update")),
            teardown: Box::new(step_failure("remove container")),
        })
    );
}
