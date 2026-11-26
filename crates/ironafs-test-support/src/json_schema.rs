// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! A check for versioned JSON objects printed by processes.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use crate::json::{Value, parse};

#[cfg(test)]
mod tests;

/// Why a process's JSON output did not match its declared object schema.
#[derive(Debug, PartialEq, Eq)]
pub enum JsonSchemaError {
    /// The JSON reader rejected the document.
    InvalidJson(String),
    /// The document's top-level value is not an object.
    ExpectedObject,
    /// A required member, including the version member, is absent.
    MissingField(String),
    /// The object has a member outside the declared schema.
    UnexpectedField(String),
    /// A decoded member name occurs more than once.
    DuplicateField(String),
    /// The version member is not the declared unsigned integer.
    WrongVersion,
    /// A field's value does not have the declared shape.
    WrongType(String),
}

impl fmt::Display for JsonSchemaError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidJson(reason) => write!(formatter, "invalid JSON: {reason}"),
            Self::ExpectedObject => formatter.write_str("expected a JSON object"),
            Self::MissingField(name) => write!(formatter, "missing JSON field {name:?}"),
            Self::UnexpectedField(name) => write!(formatter, "unexpected JSON field {name:?}"),
            Self::DuplicateField(name) => write!(formatter, "duplicate JSON field {name:?}"),
            Self::WrongVersion => formatter.write_str("unexpected JSON schema version"),
            Self::WrongType(name) => write!(formatter, "wrong type for JSON field {name:?}"),
        }
    }
}

impl Error for JsonSchemaError {}

/// The supported shapes for a declared JSON field.
#[derive(Clone, Copy, Debug)]
pub enum JsonShape<'schema> {
    /// Any JSON string.
    String,
    /// An unsigned integer representable as a `u64`.
    UnsignedInteger,
    /// An object with exactly the declared required fields.
    Object(&'schema [(&'schema str, Self)]),
}

impl JsonShape<'_> {
    fn check(&self, name: &str, value: &Value) -> Result<(), JsonSchemaError> {
        match (self, value) {
            (Self::String, Value::String(_)) => Ok(()),
            (Self::UnsignedInteger, Value::Number(text)) if text.parse::<u64>().is_ok() => Ok(()),
            (Self::Object(fields), Value::Object(members)) => check_members(members, fields, None),
            _ => Err(JsonSchemaError::WrongType(name.to_owned())),
        }
    }
}

/// Checks a JSON object's declared shapes and unsigned integer version.
///
/// Supply required fields other than the top-level version member in
/// `fields`. Member order is immaterial, including in nested objects.
///
/// # Errors
///
/// Returns a [`JsonSchemaError`] if the document does not match the declared
/// version, exact field names or field shapes.
pub fn check_json_schema(
    text: &str,
    version_field: &str,
    version: u64,
    fields: &[(&str, JsonShape<'_>)],
) -> Result<(), JsonSchemaError> {
    let document = parse(text).map_err(|error| JsonSchemaError::InvalidJson(error.to_string()))?;
    let Value::Object(members) = document else {
        return Err(JsonSchemaError::ExpectedObject);
    };
    check_members(&members, fields, Some((version_field, version)))
}

fn check_members(
    members: &[(String, Value)],
    fields: &[(&str, JsonShape<'_>)],
    version: Option<(&str, u64)>,
) -> Result<(), JsonSchemaError> {
    let mut names = BTreeSet::new();
    let mut found_version = None;
    for (name, value) in members {
        if !names.insert(name.as_str()) {
            return Err(JsonSchemaError::DuplicateField(name.clone()));
        }
        if version.is_some_and(|(field, _)| field == name) {
            found_version = Some(value);
        } else if let Some((_, shape)) = fields.iter().find(|(field, _)| *field == name) {
            shape.check(name, value)?;
        } else {
            return Err(JsonSchemaError::UnexpectedField(name.clone()));
        }
    }

    if let Some((field, expected)) = version {
        let Some(found) = found_version else {
            return Err(JsonSchemaError::MissingField(field.to_owned()));
        };
        if !matches!(found, Value::Number(text) if text.parse::<u64>() == Ok(expected)) {
            return Err(JsonSchemaError::WrongVersion);
        }
    }
    if let Some((field, _)) = fields.iter().find(|(field, _)| !names.contains(field)) {
        return Err(JsonSchemaError::MissingField((*field).to_owned()));
    }
    Ok(())
}
