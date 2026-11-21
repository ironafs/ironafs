// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! An in-memory source tree and a scripted test runner, for the unit
//! tests and `plant --self-test`: the probe's logic runs on them exactly
//! as on the real tree and nextest.

use std::collections::BTreeMap;

use super::{Files, Runner, TestOutcome};
use crate::Error;

/// Files held in memory. Writes can be set to be ignored, to plant a
/// write that did not take or a restore that did not happen.
#[derive(Debug, Default)]
pub(super) struct FakeFiles {
    files: BTreeMap<String, String>,
    writes: usize,
    /// Writes after this many are ignored.
    honoured: Option<usize>,
}

impl FakeFiles {
    pub(super) fn with(path: &str, text: &str) -> Self {
        let mut files = BTreeMap::new();
        files.insert(path.to_owned(), text.to_owned());
        Self {
            files,
            writes: 0,
            honoured: None,
        }
    }

    /// Ignores every write.
    pub(super) fn ignore_writes(&mut self) {
        self.honoured = Some(0);
    }

    /// Honours the first `count` writes and ignores the rest.
    pub(super) fn ignore_writes_after(&mut self, count: usize) {
        self.honoured = Some(count);
    }

    pub(super) fn text(&self, path: &str) -> &str {
        self.files.get(path).map_or("", String::as_str)
    }

    #[cfg(test)]
    pub(super) fn writes(&self) -> usize {
        self.writes
    }
}

impl Files for FakeFiles {
    fn read(&mut self, path: &str) -> Result<String, String> {
        self.files
            .get(path)
            .cloned()
            .ok_or_else(|| format!("{path}: not found"))
    }

    fn write(&mut self, path: &str, text: &str) -> Result<(), Error> {
        self.writes += 1;
        if self.honoured.is_none_or(|honoured| self.writes <= honoured) {
            self.files.insert(path.to_owned(), text.to_owned());
        }
        Ok(())
    }
}

/// A runner that returns its outcomes in order and records what the file
/// held at each call.
#[derive(Debug)]
pub(super) struct ScriptedRunner {
    outcomes: Vec<TestOutcome>,
    calls: usize,
    planted_texts: Vec<String>,
}

impl ScriptedRunner {
    pub(super) fn new(outcomes: &[TestOutcome]) -> Self {
        Self {
            outcomes: outcomes.to_vec(),
            calls: 0,
            planted_texts: Vec::new(),
        }
    }

    #[cfg(test)]
    pub(super) fn calls(&self) -> usize {
        self.calls
    }

    #[cfg(test)]
    /// Whether some call ran while the file held `text`.
    pub(super) fn saw_planted(&self, text: &str) -> bool {
        self.planted_texts.iter().any(|seen| seen.contains(text))
    }
}

impl Runner for ScriptedRunner {
    fn run(&mut self, _package: &str, _test: &str, file_now: &str) -> Result<TestOutcome, Error> {
        self.planted_texts.push(file_now.to_owned());
        let outcome = self
            .outcomes
            .get(self.calls)
            .cloned()
            .unwrap_or_else(|| TestOutcome::Other("the script ran out".to_owned()));
        self.calls += 1;
        Ok(outcome)
    }
}
