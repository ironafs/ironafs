// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Controls for repository automation's JSON accessors.

use super::{Value, parse};

#[test]
fn accessors_keep_the_first_decoded_member_and_handle_other_shapes() {
    let value = parse(r#"{"name":"first","na\u006de":"second","array":["one"]}"#).unwrap();

    assert_eq!(value.get("name").and_then(Value::as_str), Some("first"));
    assert_eq!(
        value.get("array").and_then(Value::as_array),
        Some([Value::String("one".into())].as_slice())
    );
    assert_eq!(value.get("missing"), None);
    assert_eq!(value.as_str(), None);
    assert_eq!(value.as_array(), None);
    assert_eq!(Value::Null.get("name"), None);
}
