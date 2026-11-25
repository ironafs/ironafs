// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The emitting rustc invocation's assembly shards and their symbol index.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use crate::Error;

use super::capture::refusal;
use super::options::Invocation;
use super::{HARNESS, label, names};

#[cfg(test)]
mod tests;

const CASES: [&str; 4] = ["wiped_live", "wiped_dead", "plain_dead", "untouched_dead"];

pub(super) fn collect(emitter: &Invocation) -> Result<Vec<String>, Error> {
    if emitter.name != HARNESS || emitter.extra_filename.is_empty() {
        return Err(refusal(
            "the assembly invocation has no harness filename identity",
        ));
    }
    let dir = &emitter.out_dir;
    let resolved =
        fs::canonicalize(dir).map_err(Error::path_io("resolve assembly output directory", dir))?;
    if resolved != *dir {
        return Err(refusal(
            "assembly output directory resolves outside its recorded path",
        ));
    }
    read_assembly(dir, &emitter.output_stem())
}

fn read_assembly(dir: &Path, stem: &str) -> Result<Vec<String>, Error> {
    let action = "list assembly output directory";
    let mut files = Vec::new();
    for entry in fs::read_dir(dir).map_err(Error::path_io(action, dir))? {
        let entry = entry.map_err(Error::path_io(action, dir))?;
        let name = entry.file_name();
        let matches = name.to_str().is_some_and(|name| {
            name.strip_prefix(stem)
                .is_some_and(|suffix| suffix.starts_with('.'))
                && Path::new(name)
                    .extension()
                    .is_some_and(|extension| extension == "s")
        });
        if matches {
            if !entry
                .file_type()
                .map_err(Error::path_io(action, &entry.path()))?
                .is_file()
            {
                return Err(refusal("assembly shard is not a regular file"));
            }
            files.push(entry.path());
        }
    }
    if files.is_empty() {
        return Err(refusal(
            "no assembly shards for the captured harness invocation",
        ));
    }
    files.sort();
    files
        .iter()
        .map(|file| fs::read_to_string(file).map_err(Error::path_io("read assembly", file)))
        .collect()
}

enum Symbol<'a> {
    Body(&'a str),
    Alias(&'a str),
}

pub(super) struct Symbols<'a> {
    entries: BTreeMap<&'a str, Symbol<'a>>,
    cases: BTreeMap<&'static str, &'a str>,
}

impl<'a> Symbols<'a> {
    pub(super) fn index(shards: impl IntoIterator<Item = &'a str>) -> Result<Self, &'static str> {
        let mut entries = BTreeMap::new();
        let mut cases = BTreeMap::new();
        for shard in shards {
            // A body borrows the remainder of its own defining shard.
            let mut tail = shard;
            while !tail.is_empty() {
                let (line, rest) = tail.split_once('\n').unwrap_or((tail, ""));
                tail = rest;
                let line = line.trim_end_matches('\r');
                let entry = if let Some(symbol) = nonlocal_label(line) {
                    Some((symbol, Symbol::Body(tail)))
                } else {
                    alias(line).map(|(name, target)| (name, Symbol::Alias(target)))
                };
                if let Some((name, symbol)) = entry {
                    if entries.insert(name, symbol).is_some() {
                        return Err("duplicate assembly symbol definition");
                    }
                    for case in CASES {
                        if names(name, case) && cases.insert(case, name).is_some() {
                            return Err("duplicate assembly symbol for a harness function");
                        }
                    }
                }
            }
        }
        Ok(Self { entries, cases })
    }

    pub(super) fn body(&self, case: &str) -> Result<Vec<&'a str>, &'static str> {
        let mut symbol = *self.cases.get(case).ok_or("no function in the assembly")?;
        let mut seen = BTreeSet::new();
        loop {
            if !seen.insert(symbol) {
                return Err("cyclic assembly alias");
            }
            match self.entries.get(symbol) {
                Some(Symbol::Body(tail)) => return body_lines(tail),
                Some(Symbol::Alias(target)) => symbol = target,
                None => return Err("unresolved assembly alias target"),
            }
        }
    }
}

fn nonlocal_label(line: &str) -> Option<&str> {
    label(line).filter(|symbol| !symbol.starts_with(".L") && symbol_name(symbol))
}

fn symbol_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'$'))
}

fn alias(line: &str) -> Option<(&str, &str)> {
    let (name, target) = line.split_once(" = ")?;
    let target = target.trim();
    (symbol_name(name) && !name.starts_with(".L") && symbol_name(target)).then_some((name, target))
}

fn body_lines(tail: &str) -> Result<Vec<&str>, &'static str> {
    let mut body = Vec::new();
    for line in tail.lines() {
        if label(line).is_some_and(|symbol| symbol.starts_with(".Lfunc_end"))
            || line.split_whitespace().next() == Some(".size")
            || nonlocal_label(line).is_some()
        {
            if body.is_empty() {
                return Err("assembly symbol has no body");
            }
            return Ok(body);
        }
        body.push(line);
    }
    Err("assembly function has no boundary")
}
