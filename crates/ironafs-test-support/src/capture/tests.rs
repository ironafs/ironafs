// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for catching panics and reading their messages.

use std::panic;

use super::catch_quietly;

#[test]
fn catch_quietly_returns_value_without_panic() {
    assert_eq!(catch_quietly(|| 7), Ok(7));
}

#[test]
fn catch_quietly_reads_static_message() {
    assert_eq!(
        catch_quietly(|| -> () { panic!("static message") }),
        Err("static message".to_owned())
    );
}

#[test]
fn catch_quietly_reads_formatted_message() {
    let value = 42;
    assert_eq!(
        catch_quietly(|| assert_eq!(value, 41, "value was {value}")),
        Err("assertion `left == right` failed: value was 42\n  left: 42\n right: 41".to_owned())
    );
}

#[test]
fn catch_quietly_names_non_string_payload() {
    assert_eq!(
        catch_quietly(|| panic::panic_any(5_u8)),
        Err("a panic whose payload is not a string".to_owned())
    );
}

#[test]
fn catch_quietly_nested_restores_outer_state() {
    let outer = catch_quietly(|| {
        let inner = catch_quietly(|| -> () { panic!("inner") });
        assert_eq!(inner, Err("inner".to_owned()));
        panic!("outer");
    });
    assert_eq!(outer, Err::<(), _>("outer".to_owned()));
}
