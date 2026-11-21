// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for catching panics and reading their messages.

use std::panic;

use super::{Panic, catch_quietly};

/// The message of the panic `run` raised.
#[track_caller]
fn message<R: std::fmt::Debug>(run: impl FnOnce() -> R) -> String {
    catch_quietly(run).unwrap_err().message
}

#[test]
fn catch_quietly_returns_value_without_panic() {
    assert_eq!(catch_quietly(|| 7), Ok(7));
}

#[test]
fn catch_quietly_records_panic_location() {
    let line = line!() + 1;
    let caught = catch_quietly(|| -> () { panic!("here") });
    assert_eq!(
        caught,
        Err(Panic {
            message: "here".to_owned(),
            location: Some(format!("{}:{line}:43", file!())),
        })
    );
}

#[test]
fn catch_quietly_reads_static_message() {
    assert_eq!(
        message(|| -> () { panic!("static message") }),
        "static message"
    );
}

#[test]
fn catch_quietly_reads_formatted_message() {
    let value = 42;
    assert_eq!(
        message(|| assert_eq!(value, 41, "value was {value}")),
        "assertion `left == right` failed: value was 42\n  left: 42\n right: 41"
    );
}

#[test]
fn catch_quietly_names_non_string_payload() {
    assert_eq!(
        message(|| panic::panic_any(5_u8)),
        "a panic whose payload is not a string"
    );
}

#[test]
fn catch_quietly_nested_restores_outer_state() {
    let outer = catch_quietly(|| {
        let inner = message(|| -> () { panic!("inner") });
        assert_eq!(inner, "inner");
        panic!("outer");
    });
    assert_eq!(outer.unwrap_err().message, "outer");
}
