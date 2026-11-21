// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The tool's JSON export: LLVM's `llvm-cov export` format with the tool's
//! version added. This is the only thing `cargo xtask coverage` reads from
//! the tool, so replacing the tool means replacing what writes this file.
//!
//! The export is input from another program: every field is checked, and a
//! missing or mistyped one makes the export malformed rather than zero.

use crate::json::{self, Value};

#[cfg(test)]
mod tests;

/// One coverage measure: how many items there are and how many ran.
#[derive(Debug, Default, PartialEq, Eq, Clone, Copy)]
pub(super) struct Counts {
    pub(super) count: u64,
    pub(super) covered: u64,
}

impl Counts {
    pub(super) fn add(&mut self, other: Self) {
        self.count = self.count.saturating_add(other.count);
        self.covered = self.covered.saturating_add(other.covered);
    }

    pub(super) fn uncovered(self) -> u64 {
        self.count.saturating_sub(self.covered)
    }
}

/// One source file's summary, with its absolute path as the tool wrote it.
#[derive(Debug)]
pub(super) struct FileSummary {
    pub(super) path: String,
    pub(super) lines: Counts,
    pub(super) regions: Counts,
    /// Functions, with a generic's instantiations counted once (LLVM groups
    /// them by where they start).
    pub(super) functions: Counts,
}

/// One function record. A generic function has one record per
/// instantiation and per binary; they share `file`, `line` and `column`.
#[derive(Debug)]
pub(super) struct FunctionRecord {
    pub(super) file: String,
    pub(super) line: u64,
    pub(super) column: u64,
    pub(super) executed: bool,
}

/// What `cargo xtask coverage` uses from the export.
#[derive(Debug)]
pub(super) struct Export {
    /// The tool's own version, from the `cargo_llvm_cov` object it adds.
    pub(super) tool_version: String,
    pub(super) files: Vec<FileSummary>,
    pub(super) functions: Vec<FunctionRecord>,
}

/// Why an export cannot be used.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum ExportError {
    /// No text, or no file measured.
    Empty,
    /// Not the expected format; the text says where.
    Malformed(String),
}

const EXPORT_TYPE: &str = "llvm.coverage.json.export";

/// Reads an export.
///
/// # Errors
///
/// Returns [`ExportError::Empty`] for an export with no text or no file, and
/// [`ExportError::Malformed`] for anything else that is not a full (not
/// summary-only) export of the expected type with the tool's version.
pub(super) fn parse(text: &str) -> Result<Export, ExportError> {
    if text.trim().is_empty() {
        return Err(ExportError::Empty);
    }
    let value = json::parse(text).map_err(|error| malformed(format!("not JSON: {error}")))?;
    if value.get("type").and_then(Value::as_str) != Some(EXPORT_TYPE) {
        return Err(malformed(format!("`type` is not \"{EXPORT_TYPE}\"")));
    }
    let tool_version = value
        .get("cargo_llvm_cov")
        .and_then(|tool| tool.get("version"))
        .and_then(Value::as_str)
        .ok_or_else(|| malformed("no `cargo_llvm_cov.version`"))?
        .to_owned();
    let Some([data]) = value.get("data").and_then(Value::as_array) else {
        return Err(malformed("`data` is not an array of exactly one export"));
    };
    let files = array(data, "files", "data")?
        .iter()
        .map(file_summary)
        .collect::<Result<Vec<_>, _>>()?;
    if files.is_empty() {
        return Err(ExportError::Empty);
    }
    let functions = data
        .get("functions")
        .and_then(Value::as_array)
        .ok_or_else(|| malformed("no `functions` array: a summary-only export"))?
        .iter()
        .map(function_record)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Export {
        tool_version,
        files,
        functions,
    })
}

fn malformed(reason: impl Into<String>) -> ExportError {
    ExportError::Malformed(reason.into())
}

fn array<'a>(value: &'a Value, key: &str, within: &str) -> Result<&'a [Value], ExportError> {
    value
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| malformed(format!("no `{key}` array in {within}")))
}

fn number(value: &Value, what: &str) -> Result<u64, ExportError> {
    match value {
        Value::Number(text) => text
            .parse()
            .map_err(|_| malformed(format!("{what} is not a count: {text}"))),
        _ => Err(malformed(format!("{what} is not a number"))),
    }
}

fn counts(summary: &Value, key: &str, path: &str) -> Result<Counts, ExportError> {
    let measure = summary
        .get(key)
        .ok_or_else(|| malformed(format!("{path}: no `{key}` summary")))?;
    let field = |name: &str| {
        measure
            .get(name)
            .ok_or_else(|| malformed(format!("{path}: no `{key}.{name}`")))
            .and_then(|value| number(value, &format!("{path}: `{key}.{name}`")))
    };
    let counts = Counts {
        count: field("count")?,
        covered: field("covered")?,
    };
    if counts.covered > counts.count {
        return Err(malformed(format!(
            "{path}: `{key}` has {} covered of {}",
            counts.covered, counts.count
        )));
    }
    Ok(counts)
}

fn file_summary(file: &Value) -> Result<FileSummary, ExportError> {
    let path = file
        .get("filename")
        .and_then(Value::as_str)
        .ok_or_else(|| malformed("a file without `filename`"))?
        .to_owned();
    let summary = file
        .get("summary")
        .ok_or_else(|| malformed(format!("{path}: no `summary`")))?;
    Ok(FileSummary {
        lines: counts(summary, "lines", &path)?,
        regions: counts(summary, "regions", &path)?,
        functions: counts(summary, "functions", &path)?,
        path,
    })
}

/// A region is `[line start, column start, line end, column end, count,
/// file id, expanded file id, kind]`; file id 0 is the function's own file,
/// `filenames[0]`, and the function starts at the earliest of its regions
/// there: the start position LLVM groups instantiations by
/// (`FunctionInstantiationSetCollector` in its `CoverageMapping.cpp`).
fn function_record(function: &Value) -> Result<FunctionRecord, ExportError> {
    let name = function
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| malformed("a function without `name`"))?;
    let executed = number(
        function
            .get("count")
            .ok_or_else(|| malformed(format!("function {name}: no `count`")))?,
        &format!("function {name}: `count`"),
    )? > 0;
    let file = array(function, "filenames", name)?
        .first()
        .and_then(Value::as_str)
        .ok_or_else(|| malformed(format!("function {name}: no file name")))?
        .to_owned();
    let mut start = None;
    for region in array(function, "regions", name)? {
        let fields = match region.as_array() {
            Some(fields) if fields.len() == 8 => fields
                .iter()
                .map(|field| number(field, &format!("function {name}: region field")))
                .collect::<Result<Vec<_>, _>>()?,
            _ => {
                return Err(malformed(format!(
                    "function {name}: a region is not 8 numbers"
                )));
            }
        };
        if let [line, column, _, _, _, 0, _, _] = fields.as_slice() {
            start = Some(start.map_or((*line, *column), |earliest: (u64, u64)| {
                earliest.min((*line, *column))
            }));
        }
    }
    let (line, column) =
        start.ok_or_else(|| malformed(format!("function {name}: no region in its own file")))?;
    Ok(FunctionRecord {
        file,
        line,
        column,
        executed,
    })
}
