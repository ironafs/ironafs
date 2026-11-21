// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The crate's target allowlist, evaluated against every target the pinned
//! compiler knows (ADR 0005, point 5.3).
//!
//! The crate root's `#[cfg(not(<guard>))] compile_error!(...)` is read from
//! its tokens, and `<guard>` (`all`, `any`, `not`, names and `key =
//! "value"` predicates) is evaluated on each target's cfg set, as `rustc
//! --print cfg --target <triple>` reports it. The targets it accepts must
//! be exactly the allowlist, except a target whose whole cfg set equals an
//! allowlisted target's: source cannot tell the two apart (only a build
//! script could read the triple, and the crate has none, ADR 0005, point
//! 1.4), so such a target is reported by name, never silently accepted. A
//! target that differs in any cfg value (another vendor, a 32-bit ABI,
//! big-endian, another ABI) must be refused by the build itself.

use std::collections::BTreeSet;

use crate::rust_source::{Token, TokenKind, ident, matching, punct};

#[cfg(test)]
mod tests;

/// A target's cfg set: each name, with its value when it has one.
pub(crate) type Cfg = BTreeSet<(String, Option<String>)>;

/// A cfg predicate.
#[derive(Debug)]
pub(crate) enum Expression {
    All(Vec<Expression>),
    Any(Vec<Expression>),
    Not(Box<Expression>),
    Name(String),
    Equals(String, String),
}

/// Reads `rustc --print cfg` output.
pub(crate) fn parse_cfg(text: &str) -> Cfg {
    text.lines()
        .filter(|line| !line.is_empty())
        .map(|line| match line.split_once('=') {
            Some((key, value)) => (key.to_owned(), Some(value.trim_matches('"').to_owned())),
            None => (line.to_owned(), None),
        })
        .collect()
}

/// Whether `expression` holds on `cfg`.
pub(crate) fn accepts(expression: &Expression, cfg: &Cfg) -> bool {
    match expression {
        Expression::All(items) => items.iter().all(|item| accepts(item, cfg)),
        Expression::Any(items) => items.iter().any(|item| accepts(item, cfg)),
        Expression::Not(item) => !accepts(item, cfg),
        Expression::Name(name) => cfg.contains(&(name.clone(), None)),
        Expression::Equals(key, value) => cfg.contains(&(key.clone(), Some(value.clone()))),
    }
}

/// The guard of the crate root's `#[cfg(not(<guard>))] compile_error!`.
///
/// # Errors
///
/// Returns why the root has no such guard, or one this reader cannot
/// evaluate exactly.
pub(crate) fn guard(tokens: &[Token]) -> Result<Expression, String> {
    let call = tokens
        .iter()
        .position(|token| ident(Some(token)) == Some("compile_error"))
        .ok_or("the crate root has no compile_error!")?;
    let close = call
        .checked_sub(1)
        .ok_or("no attribute stands directly before compile_error!")?;
    let open = (0..call)
        .find(|&at| {
            punct(tokens.get(at)) == Some('#') && matching(tokens, at + 1, '[', ']') == Some(close)
        })
        .ok_or("no attribute stands directly before compile_error!")?;
    let start = open + 2;
    let (expression, next) = read(tokens, start)?;
    if next != close {
        return Err("the attribute before compile_error! holds more than one cfg".to_owned());
    }
    match expression {
        Expression::All(mut items) if items.len() == 1 => match items.pop() {
            Some(Expression::Not(guard)) => Ok(*guard),
            _ => Err("the attribute is not #[cfg(not(...))]".to_owned()),
        },
        _ => Err("the attribute is not #[cfg(not(...))]".to_owned()),
    }
}

/// Reads one predicate at `at`; `cfg(...)` reads as an `all` of its
/// contents. Returns it and the index after it.
fn read(tokens: &[Token], at: usize) -> Result<(Expression, usize), String> {
    let name = ident(tokens.get(at)).ok_or("a cfg predicate that is not a name")?;
    let line = tokens.get(at).map_or(0, |token| token.line);
    if punct(tokens.get(at + 1)) == Some('=') {
        return match tokens.get(at + 2).map(|token| &token.kind) {
            Some(TokenKind::Str(value)) => {
                Ok((Expression::Equals(name.to_owned(), value.clone()), at + 3))
            }
            _ => Err(format!("line {line}: `{name} =` without a string")),
        };
    }
    if punct(tokens.get(at + 1)) != Some('(') {
        return Ok((Expression::Name(name.to_owned()), at + 1));
    }
    let close = matching(tokens, at + 1, '(', ')').ok_or("an unclosed cfg predicate")?;
    let mut items = Vec::new();
    let mut next = at + 2;
    while next < close {
        let (item, after) = read(tokens, next)?;
        items.push(item);
        next = after;
        if punct(tokens.get(next)) == Some(',') {
            next += 1;
        }
    }
    let expression = match name {
        "cfg" | "all" => Expression::All(items),
        "any" => Expression::Any(items),
        "not" if items.len() == 1 => {
            Expression::Not(Box::new(items.pop().ok_or("an empty not()")?))
        }
        other => {
            return Err(format!(
                "line {line}: `{other}(...)` is not all, any or not"
            ));
        }
    };
    Ok((expression, close + 1))
}

/// Each target the pinned compiler knows that the crate root's guard
/// accepts, with its cfg set, read with that compiler from `root`.
///
/// # Errors
///
/// Returns an error if `rustc` cannot be run or reports failure, or the
/// root's guard cannot be read.
pub(crate) fn accepted_targets(
    root: &std::path::Path,
    lib_rs: &str,
) -> Result<Result<std::collections::BTreeMap<String, Cfg>, String>, crate::Error> {
    let tokens = match crate::rust_source::tokens_with_strings(lib_rs) {
        Ok(tokens) => tokens,
        Err(error) => return Ok(Err(error.to_string())),
    };
    let expression = match guard(&tokens) {
        Ok(expression) => expression,
        Err(error) => return Ok(Err(error)),
    };
    let rustc = |arguments: &[&str]| -> Result<String, crate::Error> {
        let output = crate::Command::new("rustc")
            .current_dir(root)
            .args(arguments)
            .output()
            .map_err(crate::Error::io("run rustc --print"))?;
        crate::Error::check_status("rustc --print", output.status)?;
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    };
    let mut accepted = std::collections::BTreeMap::new();
    for target in rustc(&["--print", "target-list"])?.lines() {
        let cfg = parse_cfg(&rustc(&["--print", "cfg", "--target", target])?);
        if accepts(&expression, &cfg) {
            accepted.insert(target.to_owned(), cfg);
        }
    }
    Ok(Ok(accepted))
}
