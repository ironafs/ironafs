// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Controls for the JSON output checker.

use super::{JsonSchemaError, JsonShape, check_json_schema};

fn check(text: &str) -> Result<(), JsonSchemaError> {
    check_json_schema(
        text,
        "schema_version",
        1,
        &[
            ("name", JsonShape::String),
            ("count", JsonShape::UnsignedInteger),
        ],
    )
}

#[test]
fn accepts_reordered_exact_fields_and_the_declared_version() {
    assert_eq!(
        check(r#"{"count":2,"name":"case","schema_version":1}"#),
        Ok(())
    );
}

#[test]
fn refuses_a_wrong_version_and_noninteger_versions() {
    for version in ["2", r#""1""#, "1.0", "1e0"] {
        let text = format!(r#"{{"schema_version":{version},"name":"case","count":2}}"#);
        assert_eq!(check(&text), Err(JsonSchemaError::WrongVersion), "{text}");
    }
}

#[test]
fn refuses_a_missing_field() {
    assert_eq!(
        check(r#"{"schema_version":1,"name":"case"}"#),
        Err(JsonSchemaError::MissingField("count".into()))
    );
}

#[test]
fn refuses_an_extra_field() {
    assert_eq!(
        check(r#"{"schema_version":1,"name":"case","count":2,"extra":null}"#),
        Err(JsonSchemaError::UnexpectedField("extra".into()))
    );
}

#[test]
fn refuses_decoded_duplicate_fields_and_duplicate_versions() {
    for (text, key) in [
        (
            r#"{"schema_version":1,"name":"case","count":2,"co\u0075nt":2}"#,
            "count",
        ),
        (
            r#"{"schema_version":1,"name":"case","count":2,"schema_version":1}"#,
            "schema_version",
        ),
    ] {
        assert_eq!(
            check(text),
            Err(JsonSchemaError::DuplicateField(key.into()))
        );
    }
}

#[test]
fn refuses_same_size_object_with_a_replaced_field() {
    assert_eq!(
        check(r#"{"schema_version":1,"name":"case","extra":2}"#),
        Err(JsonSchemaError::UnexpectedField("extra".into()))
    );
}

#[test]
fn refuses_missing_version_nonobject_and_malformed_document() {
    assert_eq!(
        check(r#"{"name":"case","count":2}"#),
        Err(JsonSchemaError::MissingField("schema_version".into()))
    );
    assert_eq!(check("[1, 2]"), Err(JsonSchemaError::ExpectedObject));
    assert!(matches!(
        check(r#"{"schema_version":1}{}"#),
        Err(JsonSchemaError::InvalidJson(_))
    ));
}

fn check_details(text: &str) -> Result<(), JsonSchemaError> {
    check_json_schema(
        text,
        "schema_version",
        1,
        &[(
            "details",
            JsonShape::Object(&[("label", JsonShape::String)]),
        )],
    )
}

#[test]
fn shape_followup_rejects_wrong_member_types() {
    assert_eq!(
        check(r#"{"schema_version":1,"name":123,"count":"x"}"#),
        Err(JsonSchemaError::WrongType("name".into()))
    );
    for value in [r#""x""#, "-1", "2.0"] {
        let text = format!(r#"{{"schema_version":1,"name":"case","count":{value}}}"#);
        assert_eq!(
            check(&text),
            Err(JsonSchemaError::WrongType("count".into())),
            "{text}"
        );
    }
}

#[test]
fn shape_followup_requires_a_nested_object() {
    assert_eq!(
        check_details(r#"{"schema_version":1,"details":"not an object"}"#),
        Err(JsonSchemaError::WrongType("details".into()))
    );
}

#[test]
fn shape_followup_requires_nested_member_name_and_type() {
    for (details, error) in [
        (
            r#"{"label":12}"#,
            JsonSchemaError::WrongType("label".into()),
        ),
        ("{}", JsonSchemaError::MissingField("label".into())),
        (
            r#"{"other":"case"}"#,
            JsonSchemaError::UnexpectedField("other".into()),
        ),
        (
            r#"{"label":"case","lab\u0065l":"duplicate"}"#,
            JsonSchemaError::DuplicateField("label".into()),
        ),
    ] {
        let text = format!(r#"{{"schema_version":1,"details":{details}}}"#);
        assert_eq!(check_details(&text), Err(error), "{text}");
    }
}

#[test]
fn shape_followup_accepts_valid_nested_object() {
    assert_eq!(
        check_details(r#"{"details":{"label":"case"},"schema_version":1}"#),
        Ok(())
    );
}
