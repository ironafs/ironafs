// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The crate root's `compile_error!` guard, evaluated against every target
//! the pinned compiler knows (ADR 0005, point 5.3).
//!
//! The crate root's `#[cfg(not(<guard>))] compile_error!(...)` is read from
//! its tokens, and `<guard>` (`all`, `any`, `not`, names and `key =
//! "value"` predicates) is evaluated on each target's cfg set, as `rustc
//! --print cfg --target <triple>` reports it. The verdict it must give is
//! derived from point 5.3's list, never from the guard: a target passes
//! exactly when its whole cfg set equals a listed target's. The guard is
//! the second net; the target gate in the build script (`gate`) is the part
//! that refuses, by triple, the targets whose cfg set equals a listed one's.

use std::collections::BTreeSet;

use super::pin::Pinned;
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
    top_level_alone(tokens, open)?;
    no_conditional_crate_attribute(tokens)?;
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

/// Refuses a crate-level inner attribute (`#![...]`, at the top level of
/// the crate root) that names `cfg` or `cfg_attr` anywhere in it: such an
/// attribute can remove the whole crate, the guard with it, on targets the
/// guard itself refuses (`#![cfg(...)]`, or `#![cfg_attr(..., cfg(...))]`),
/// and this reader does not evaluate it.
fn no_conditional_crate_attribute(tokens: &[Token]) -> Result<(), String> {
    let mut depth = 0_i64;
    for (at, token) in tokens.iter().enumerate() {
        match punct(Some(token)) {
            Some('{') => depth += 1,
            Some('}') => depth -= 1,
            Some('#') if depth == 0 && punct(tokens.get(at + 1)) == Some('!') => {
                let close = matching(tokens, at + 2, '[', ']')
                    .ok_or("an unbalanced crate-level attribute")?;
                let conditional = tokens
                    .get(at + 3..close)
                    .unwrap_or_default()
                    .iter()
                    .any(|inner| matches!(ident(Some(inner)), Some("cfg" | "cfg_attr")));
                if conditional {
                    return Err(format!(
                        "line {}: a crate-level attribute names cfg or cfg_attr, and could \
                         remove the crate and its compile_error! guard on a target the guard \
                         refuses",
                        token.line
                    ));
                }
            }
            _ => {}
        }
    }
    Ok(())
}

/// Checks that the guard attribute at `open` stands at the top level of
/// the crate root (outside every brace) and alone on its item: before it
/// comes only the start of the file, the end of another item (`;` or
/// `}`), or an inner attribute (`#![...]`). Another outer attribute there,
/// such as `#[cfg(any())]`, could remove the `compile_error!` on a target
/// the guard itself refuses.
fn top_level_alone(tokens: &[Token], open: usize) -> Result<(), String> {
    let mut depth = 0_i64;
    for token in tokens.get(..open).unwrap_or_default() {
        match punct(Some(token)) {
            Some('{') => depth += 1,
            Some('}') => depth -= 1,
            _ => {}
        }
    }
    if depth != 0 {
        return Err(
            "the compile_error! guard is not at the top level of the crate root".to_owned(),
        );
    }
    let Some(before) = open.checked_sub(1) else {
        return Ok(());
    };
    match punct(tokens.get(before)) {
        Some(';' | '}') => Ok(()),
        Some(']') => {
            let start = (0..before)
                .rev()
                .find(|&at| matching(tokens, at, '[', ']') == Some(before))
                .ok_or("an unbalanced attribute before the guard")?;
            let inner = start.checked_sub(2).is_some_and(|hash| {
                punct(tokens.get(hash)) == Some('#') && punct(tokens.get(hash + 1)) == Some('!')
            });
            if inner {
                Ok(())
            } else {
                Err(
                    "another attribute stands on the compile_error! guard's item, and could \
                     remove it"
                        .to_owned(),
                )
            }
        }
        _ => Err("the compile_error! guard does not start its own item".to_owned()),
    }
}

/// The cfg keys whose values differ between a musl target and the glibc
/// target of the same architecture, as the pinned compiler reports them
/// (`target_env`, and `target_feature`'s `crt-static`). Naming any of them
/// in the crate outside its allowlist could make a musl build compile other
/// code than the glibc build, which the copy's equal statuses assume it
/// does not.
///
/// # Errors
///
/// Returns an error if `rustc` cannot be run.
pub(crate) fn musl_keys(
    pinned: &Pinned,
    root: &std::path::Path,
) -> Result<BTreeSet<String>, crate::Error> {
    let mut keys = BTreeSet::new();
    for (musl, gnu) in [
        ("x86_64-unknown-linux-musl", "x86_64-unknown-linux-gnu"),
        ("aarch64-unknown-linux-musl", "aarch64-unknown-linux-gnu"),
    ] {
        let musl = parse_cfg(&print_cfg(pinned, root, musl)?);
        let gnu = parse_cfg(&print_cfg(pinned, root, gnu)?);
        keys.extend(musl.symmetric_difference(&gnu).map(|(key, _)| key.clone()));
    }
    Ok(keys)
}

fn print_cfg(
    pinned: &Pinned,
    root: &std::path::Path,
    target: &str,
) -> Result<String, crate::Error> {
    print(pinned, root, &["--print", "cfg", "--target", target])
}

/// What the pinned compiler, run from `root`, prints for `arguments`.
fn print(
    pinned: &Pinned,
    root: &std::path::Path,
    arguments: &[&str],
) -> Result<String, crate::Error> {
    let output = crate::Command::new(&pinned.rustc)
        .current_dir(root)
        .args(arguments)
        .output()
        .map_err(crate::Error::io("run rustc --print"))?;
    crate::Error::check_status("rustc --print", output.status)?;
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
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

/// The guard of the crate root `lib_rs`.
///
/// # Errors
///
/// Returns why the root cannot be read, or has no guard this reader can
/// evaluate exactly.
pub(crate) fn guard_of(lib_rs: &str) -> Result<Expression, String> {
    let tokens =
        crate::rust_source::tokens_with_strings(lib_rs).map_err(|error| error.to_string())?;
    guard(&tokens)
}

/// Every target the pinned compiler knows, with its cfg set, read with that
/// compiler from `root`.
///
/// # Errors
///
/// Returns an error if `rustc` cannot be run or reports failure.
pub(crate) fn target_cfgs(
    pinned: &Pinned,
    root: &std::path::Path,
) -> Result<std::collections::BTreeMap<String, Cfg>, crate::Error> {
    let mut cfgs = std::collections::BTreeMap::new();
    for target in print(pinned, root, &["--print", "target-list"])?.lines() {
        let cfg = parse_cfg(&print_cfg(pinned, root, target)?);
        cfgs.insert(target.to_owned(), cfg);
    }
    Ok(cfgs)
}
