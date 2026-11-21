// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The test inventory ratchet (`docs/specs/testing.md`, 5.4, point 5).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::io::Write;
use std::path::Path;

use crate::Error;
use crate::args::InventoryMode;
use crate::junit::Case;
use crate::metadata;

mod listing;
mod self_test;
mod source;

#[cfg(test)]
mod tests;

/// The targets an inventory line may name. A line with no `target=` tag
/// applies to each of them; a run on any other target cannot be checked.
pub(crate) const TARGETS: &[&str] = &[
    "aarch64-apple-darwin",
    "aarch64-unknown-linux-gnu",
    "x86_64-unknown-linux-gnu",
];

/// The targets whose entries a CI job reconciles, the only ones a
/// `target=` tag may name: a line tagged for a target no job runs would
/// never be checked. The change that adds a CI job running `cargo xtask
/// inventory` on another target adds that target here.
pub(crate) const RECONCILED_TARGETS: &[&str] = &["x86_64-unknown-linux-gnu"];

/// The feature sets a line may name. A line with no `features=` tag
/// belongs to the first, the one every run of `cargo xtask inventory` uses.
pub(crate) const FEATURE_SETS: &[&str] = &["default"];

/// Where the inventory lives, relative to the workspace root.
const DIR: &str = "tests/inventory";

/// The removal record's file name, beside the per-crate files.
pub(crate) const REMOVED: &str = "removed.txt";

/// The inventory files of one revision: each crate's file by crate name,
/// and the removal record if it exists.
#[derive(Debug)]
pub(crate) struct Files {
    pub(crate) inventories: Vec<(String, String)>,
    pub(crate) removed: Option<String>,
}

/// One test nextest listed, ignored and filtered tests included.
#[derive(Debug)]
pub(crate) struct Listed {
    pub(crate) package: String,
    pub(crate) binary: String,
    pub(crate) name: String,
}

/// What one run listed and how each test it ran ended.
#[derive(Debug)]
pub(crate) struct Run {
    pub(crate) target: String,
    pub(crate) features: String,
    pub(crate) listed: Vec<Listed>,
    pub(crate) results: Vec<Case>,
}

/// An inventory entry for one target and feature set.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Key {
    pub(crate) binary: String,
    pub(crate) name: String,
    pub(crate) target: String,
    pub(crate) features: String,
}

impl fmt::Display for Key {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} {} on {}, features {}",
            self.binary, self.name, self.target, self.features
        )
    }
}

/// A line's quarantine mark: absent, or present with its issue if it
/// cites one.
type Mark = Option<Option<u64>>;

/// One broken inventory rule.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Violation {
    Format {
        file: String,
        line: usize,
        reason: String,
    },
    QuarantineWithoutIssue {
        file: String,
        line: usize,
    },
    Unlisted {
        key: Key,
        file: String,
    },
    ListedNotRun {
        key: Key,
    },
    QuarantinedNotListed {
        key: Key,
        issue: u64,
    },
    UnrecordedRemoval {
        binary: String,
        name: String,
    },
    RemovalRecordDeleted {
        line: String,
    },
    DroppedQuarantine {
        key: Key,
        issue: u64,
    },
    QuarantineLifted {
        key: Key,
        issue: u64,
    },
}

impl fmt::Display for Violation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Format { file, line, reason } => {
                write!(formatter, "tests/inventory/{file}:{line}: {reason}")
            }
            Self::QuarantineWithoutIssue { file, line } => write!(
                formatter,
                "tests/inventory/{file}:{line}: a quarantine mark cites its issue, as \
                 `quarantined=#<issue>` (coding standards rule 15)"
            ),
            Self::Unlisted { key, file } if RECONCILED_TARGETS.contains(&key.target.as_str()) => {
                write!(
                    formatter,
                    "{key}: in the run but not in tests/inventory/{file}.txt; add the line \
                     `{} {} target={}` (or run `cargo xtask inventory update` on that target)",
                    key.binary, key.name, key.target
                )
            }
            Self::Unlisted { key, file } => write!(
                formatter,
                "{key}: in the run but not in tests/inventory/{file}.txt; add the untagged \
                 line `{} {}`, or remove the target tag from its line (run `cargo xtask \
                 inventory update`)",
                key.binary, key.name
            ),
            Self::ListedNotRun { key } => write!(
                formatter,
                "{key}: listed but did not run and pass; fix it, quarantine it with its \
                 issue, or remove its line and record the removal in tests/inventory/{REMOVED}"
            ),
            Self::UnrecordedRemoval { binary, name } => write!(
                formatter,
                "{binary} {name}: removed from the base revision's inventory with no line in \
                 tests/inventory/{REMOVED} (`{binary} {name} #<pull request> <reason>`)"
            ),
            Self::RemovalRecordDeleted { line } => write!(
                formatter,
                "tests/inventory/{REMOVED}: the base revision's line `{line}` was deleted or \
                 changed; the record only grows"
            ),
            Self::DroppedQuarantine { key, issue } => write!(
                formatter,
                "{key}: quarantined in the base revision (#{issue}) and dropped from the \
                 inventory; a quarantined test stays listed and marked"
            ),
            Self::QuarantinedNotListed { key, issue } => write!(
                formatter,
                "{key}: marked quarantined (#{issue}) but no longer listed by nextest, which \
                 lists ignored and filtered tests; a deleted test's line is removed and \
                 recorded in tests/inventory/{REMOVED}"
            ),
            Self::QuarantineLifted { key, issue } => write!(
                formatter,
                "{key}: the quarantine mark (#{issue}) was removed but the test did not run \
                 and pass in this run"
            ),
        }
    }
}

/// What a reconciliation found and what it prints.
#[derive(Debug, Default)]
pub(crate) struct Outcome {
    /// Names gone since the base revision, each with its record.
    pub(crate) removed: Vec<String>,
    /// Entries marked quarantined that the base revision did not mark.
    pub(crate) newly_quarantined: Vec<String>,
    /// Quarantined entries of this run that did not run.
    pub(crate) skipped: Vec<String>,
    /// One line per crate: its entries for this run and the change.
    pub(crate) counts: Vec<String>,
    /// How many entries of this run's target and feature set were compared.
    pub(crate) compared: usize,
    pub(crate) violations: Vec<Violation>,
}

/// One parsed line.
#[derive(Debug)]
struct Entry {
    binary: String,
    name: String,
    target: Option<String>,
    features: Option<String>,
    mark: Mark,
}

/// The crate a binary id belongs to: `<crate>` or `<crate>::<kind>/<name>`.
fn crate_of(binary: &str) -> &str {
    binary.split_once("::").map_or(binary, |(name, _)| name)
}

/// The lines of a file that must end each line with a newline. A missing
/// final newline is reported against the last line.
fn lines<'a>(file: &str, text: &'a str, violations: &mut Vec<Violation>) -> Vec<&'a str> {
    let mut lines: Vec<&str> = text.split('\n').collect();
    if lines.pop() != Some("") {
        violations.push(Violation::Format {
            file: file.to_owned(),
            line: lines.len() + 1,
            reason: "the file does not end with a newline".to_owned(),
        });
        // Keep the unterminated line so it is still read.
        if let Some(last) = text.rsplit('\n').next() {
            lines.push(last);
        }
    }
    lines
}

/// Reads one crate's file into its entries, reporting each malformed line.
/// A line that cannot be read is left out; a quarantine mark without its
/// issue is reported and the entry kept as marked.
fn parse_inventory(crate_name: &str, text: &str, violations: &mut Vec<Violation>) -> Vec<Entry> {
    let file = format!("{crate_name}.txt");
    let mut entries = Vec::new();
    let mut previous: Option<&str> = None;
    for (index, line) in lines(&file, text, violations).into_iter().enumerate() {
        let number = index + 1;
        let mut format = |reason: String| {
            violations.push(Violation::Format {
                file: file.clone(),
                line: number,
                reason,
            });
        };
        if previous.is_some_and(|previous| previous >= line) {
            format("lines are sorted and each appears once".to_owned());
        }
        previous = Some(line);
        let fields: Vec<&str> = line.split(' ').collect();
        if fields.iter().any(|field| field.is_empty()) {
            format("fields are separated by one space, with no blank lines".to_owned());
            continue;
        }
        let (Some(binary), Some(name)) = (fields.first(), fields.get(1)) else {
            format("a line is `<binary id> <test name>`, then its tags".to_owned());
            continue;
        };
        if crate_of(binary) != crate_name {
            format(format!(
                "{binary} belongs in tests/inventory/{}.txt",
                crate_of(binary)
            ));
            continue;
        }
        let mut entry = Entry {
            binary: (*binary).to_owned(),
            name: (*name).to_owned(),
            target: None,
            features: None,
            mark: None,
        };
        let mut bad = false;
        for tag in fields.iter().skip(2) {
            let (key, value) = tag.split_once('=').unwrap_or((tag, ""));
            let repeated = match key {
                "target" => entry.target.replace(value.to_owned()).is_some(),
                "features" => entry.features.replace(value.to_owned()).is_some(),
                "quarantined" => entry.mark.replace(issue(value)).is_some(),
                _ => {
                    format(format!(
                        "unknown tag `{tag}`; a tag is target=<triple>, features=<set> or \
                         quarantined=#<issue>"
                    ));
                    bad = true;
                    continue;
                }
            };
            if repeated {
                format(format!("the tag {key} appears twice"));
                bad = true;
            }
        }
        if let Some(target) = &entry.target
            && !RECONCILED_TARGETS.contains(&target.as_str())
        {
            format(format!(
                "target `{target}`: no CI job reconciles it, so a line tagged for it would \
                 never be checked; a tag may name {}",
                RECONCILED_TARGETS.join(", ")
            ));
            bad = true;
        }
        if let Some(features) = &entry.features
            && !FEATURE_SETS.contains(&features.as_str())
        {
            format(format!(
                "unknown feature set `{features}`: no run reconciles it; the feature sets \
                 are {}",
                FEATURE_SETS.join(", ")
            ));
            bad = true;
        }
        if bad {
            continue;
        }
        if entry.mark == Some(None) {
            violations.push(Violation::QuarantineWithoutIssue {
                file: file.clone(),
                line: number,
            });
        }
        entries.push((number, entry));
    }
    entries.into_iter().map(|(_, entry)| entry).collect()
}

/// The issue number of a mark's value, `#<digits>`.
fn issue(value: &str) -> Option<u64> {
    let digits = value.strip_prefix('#')?;
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok().filter(|issue| *issue > 0)
}

/// Every entry of a revision, expanded to one key per target and feature
/// set it applies to, with its mark. A key two lines give is reported.
fn expand(files: &Files, violations: &mut Vec<Violation>) -> BTreeMap<Key, Mark> {
    let mut keys = BTreeMap::new();
    for (crate_name, text) in &files.inventories {
        for entry in parse_inventory(crate_name, text, violations) {
            let targets: Vec<&str> = match &entry.target {
                Some(target) => vec![target.as_str()],
                None => TARGETS.to_vec(),
            };
            let features = entry
                .features
                .clone()
                .or_else(|| FEATURE_SETS.first().map(|set| (*set).to_owned()))
                .unwrap_or_default();
            for target in targets {
                let key = Key {
                    binary: entry.binary.clone(),
                    name: entry.name.clone(),
                    target: target.to_owned(),
                    features: features.clone(),
                };
                if keys.insert(key.clone(), entry.mark).is_some() {
                    violations.push(Violation::Format {
                        file: format!("{crate_name}.txt"),
                        line: 0,
                        reason: format!("{key} is listed by two lines"),
                    });
                }
            }
        }
    }
    keys
}

/// One well-formed line of the removal record: the line, the name it
/// records and its pull request and reason.
type Record = (String, (String, String), String);

/// The removal record's lines, each checked as
/// `<binary id> <test name> #<pull request> <reason>`, and the well-formed
/// ones as records.
fn parse_removed(text: &str, violations: &mut Vec<Violation>) -> (Vec<String>, Vec<Record>) {
    let mut kept = Vec::new();
    let mut recorded = Vec::new();
    for (index, line) in lines(REMOVED, text, violations).into_iter().enumerate() {
        kept.push(line.to_owned());
        let mut fields = line.splitn(4, ' ');
        let (Some(binary), Some(name), Some(pull), Some(reason)) =
            (fields.next(), fields.next(), fields.next(), fields.next())
        else {
            violations.push(removed_format(index + 1));
            continue;
        };
        if binary.is_empty() || name.is_empty() || issue(pull).is_none() || reason.trim().is_empty()
        {
            violations.push(removed_format(index + 1));
            continue;
        }
        recorded.push((
            line.to_owned(),
            (binary.to_owned(), name.to_owned()),
            format!("{pull} {reason}"),
        ));
    }
    (kept, recorded)
}

fn removed_format(line: usize) -> Violation {
    Violation::Format {
        file: REMOVED.to_owned(),
        line,
        reason: "a line is `<binary id> <test name> #<pull request> <reason>`".to_owned(),
    }
}

/// Reconciles the current inventory with the run and, given the base
/// revision's inventory, checks the removal and quarantine rules against
/// it. Only entries of the run's target and feature set are compared with
/// the run; the base rules cover every entry.
pub(crate) fn reconcile(current: &Files, base: Option<&Files>, run: &Run) -> Outcome {
    let mut outcome = Outcome::default();
    let keys = expand(current, &mut outcome.violations);
    let results = compare_run(&keys, run, &mut outcome);
    let base_keys =
        base.map(|base| compare_base(current, &keys, base, run, &results, &mut outcome));
    count(&keys, base_keys.as_ref(), run, &mut outcome);
    outcome
}

/// Whether an entry belongs to the run's target and feature set.
fn own(key: &Key, run: &Run) -> bool {
    key.target == run.target && key.features == run.features
}

/// Compares the entries of the run's target and feature set with what the
/// run listed and how each test ended, and returns those endings: a test
/// reported twice passed only if both passed.
fn compare_run(
    keys: &BTreeMap<Key, Mark>,
    run: &Run,
    outcome: &mut Outcome,
) -> BTreeMap<Key, bool> {
    let key_of = |binary: &str, name: &str| Key {
        binary: binary.to_owned(),
        name: name.to_owned(),
        target: run.target.clone(),
        features: run.features.clone(),
    };
    let mut results: BTreeMap<Key, bool> = BTreeMap::new();
    for case in &run.results {
        let passed = results
            .entry(key_of(&case.binary, &case.name))
            .or_insert(true);
        *passed = *passed && case.passed;
    }
    let mut in_run: BTreeMap<Key, String> = BTreeMap::new();
    for listed in &run.listed {
        in_run.insert(key_of(&listed.binary, &listed.name), listed.package.clone());
    }
    for key in results.keys() {
        in_run
            .entry(key.clone())
            .or_insert_with(|| crate_of(&key.binary).to_owned());
    }
    for (key, package) in &in_run {
        if !keys.contains_key(key) {
            outcome.violations.push(Violation::Unlisted {
                key: key.clone(),
                file: package.clone(),
            });
        }
    }
    for (key, mark) in keys.iter().filter(|(key, _)| own(key, run)) {
        outcome.compared += 1;
        match (results.get(key), mark) {
            (Some(true), _) => {}
            // nextest lists ignored and filtered tests too, so a mark
            // covers only a test that still exists.
            (None, Some(Some(issue))) if !in_run.contains_key(key) => {
                outcome.violations.push(Violation::QuarantinedNotListed {
                    key: key.clone(),
                    issue: *issue,
                });
            }
            (None, Some(issue)) => outcome.skipped.push(match issue {
                Some(issue) => format!("{key} (quarantined, #{issue})"),
                None => format!("{key} (quarantined, no issue)"),
            }),
            _ => outcome
                .violations
                .push(Violation::ListedNotRun { key: key.clone() }),
        }
    }
    results
}

/// Checks the removal record and the quarantine marks against the base
/// revision's inventory, and returns the base's entries.
fn compare_base(
    current: &Files,
    keys: &BTreeMap<Key, Mark>,
    base: &Files,
    run: &Run,
    results: &BTreeMap<Key, bool>,
    outcome: &mut Outcome,
) -> BTreeMap<Key, Mark> {
    let violations = &mut outcome.violations;
    let (removed_lines, records) =
        parse_removed(current.removed.as_deref().unwrap_or_default(), violations);
    // The base revision's own format was checked when it landed.
    let mut ignored = Vec::new();
    let base_keys = expand(base, &mut ignored);
    let (base_lines, _) = parse_removed(base.removed.as_deref().unwrap_or_default(), &mut ignored);
    // Only a line this change adds records a removal it makes: a line the
    // base already holds recorded an earlier removal of the same name.
    let recorded: BTreeMap<(String, String), String> = records
        .into_iter()
        .filter(|(line, _, _)| !base_lines.contains(line))
        .map(|(_, name, record)| (name, record))
        .collect();
    for line in base_lines {
        if !removed_lines.contains(&line) {
            violations.push(Violation::RemovalRecordDeleted { line });
        }
    }
    // Keys are ordered by binary and name first, so one name's keys are
    // adjacent.
    let mut gone: Vec<(String, String)> = base_keys
        .keys()
        .filter(|key| !keys.contains_key(*key))
        .map(|key| (key.binary.clone(), key.name.clone()))
        .collect();
    gone.dedup();
    for (binary, name) in gone {
        if let Some(record) = recorded.get(&(binary.clone(), name.clone())) {
            outcome.removed.push(format!("{binary} {name} ({record})"));
        } else {
            outcome.removed.push(format!("{binary} {name} (no record)"));
            violations.push(Violation::UnrecordedRemoval { binary, name });
        }
    }
    for (key, mark) in &base_keys {
        let Some(Some(issue)) = mark else { continue };
        match keys.get(key) {
            None => violations.push(Violation::DroppedQuarantine {
                key: key.clone(),
                issue: *issue,
            }),
            // An entry of another target or feature set is shown running by
            // that target's own run.
            Some(None) if own(key, run) && results.get(key) != Some(&true) => {
                violations.push(Violation::QuarantineLifted {
                    key: key.clone(),
                    issue: *issue,
                });
            }
            _ => {}
        }
    }
    for (key, mark) in keys {
        if let Some(Some(issue)) = mark
            && !matches!(base_keys.get(key), Some(Some(_)))
        {
            let shown = format!("{} {} (#{issue})", key.binary, key.name);
            if !outcome.newly_quarantined.contains(&shown) {
                outcome.newly_quarantined.push(shown);
            }
        }
    }
    base_keys
}

/// One line per crate: its entries of the run's target and feature set,
/// and the change against the base.
fn count(
    keys: &BTreeMap<Key, Mark>,
    base_keys: Option<&BTreeMap<Key, Mark>>,
    run: &Run,
    outcome: &mut Outcome,
) {
    let mut crates: BTreeSet<&str> = keys.keys().map(|key| crate_of(&key.binary)).collect();
    if let Some(base_keys) = base_keys {
        crates.extend(base_keys.keys().map(|key| crate_of(&key.binary)));
    }
    for name in crates {
        let mine = |key: &&Key| own(key, run) && crate_of(&key.binary) == name;
        let now: BTreeSet<&Key> = keys.keys().filter(mine).collect();
        let change = match base_keys {
            Some(base_keys) => {
                let before: BTreeSet<&Key> = base_keys.keys().filter(mine).collect();
                format!(
                    "base {}, +{} -{}",
                    before.len(),
                    now.difference(&before).count(),
                    before.difference(&now).count()
                )
            }
            None => "no base inventory".to_owned(),
        };
        outcome.counts.push(format!(
            "{name}: {} tests on {} ({change})",
            now.len(),
            run.target
        ));
    }
}

/// Adds a line for every test of the run that has no entry for the run's
/// target and feature set, keeping every existing line, and returns each
/// file that changed with its new text. A test with no line at all gets an
/// untagged line; one listed for other targets gets one for this target, or,
/// when no tag may name this target, is returned by name to be untagged by
/// hand.
pub(crate) fn update(current: &Files, run: &Run) -> (Vec<(String, String)>, Vec<String>) {
    let mut ignored = Vec::new();
    let keys = expand(current, &mut ignored);
    let mut files: BTreeMap<String, Vec<String>> = current
        .inventories
        .iter()
        .map(|(name, text)| (name.clone(), text.lines().map(str::to_owned).collect()))
        .collect();
    let mut changed = BTreeSet::new();
    let mut untag = Vec::new();
    for listed in &run.listed {
        let key = Key {
            binary: listed.binary.clone(),
            name: listed.name.clone(),
            target: run.target.clone(),
            features: run.features.clone(),
        };
        if keys.contains_key(&key) {
            continue;
        }
        let named = keys
            .keys()
            .any(|other| other.binary == key.binary && other.name == key.name);
        let mut line = format!("{} {}", key.binary, key.name);
        if named {
            if !RECONCILED_TARGETS.contains(&key.target.as_str()) {
                // No tag may name this target: the line must lose its tag.
                untag.push(line);
                continue;
            }
            line = format!("{line} target={}", key.target);
        }
        if FEATURE_SETS.first() != Some(&key.features.as_str()) {
            line = format!("{line} features={}", key.features);
        }
        let lines = files.entry(listed.package.clone()).or_default();
        if !lines.contains(&line) {
            lines.push(line);
            changed.insert(listed.package.clone());
        }
    }
    let written = changed
        .into_iter()
        .filter_map(|name| {
            let mut lines = files.remove(&name)?;
            lines.sort();
            lines.dedup();
            lines.push(String::new());
            Some((name, lines.join("\n")))
        })
        .collect();
    (written, untag)
}

const CHECK: &str = "inventory";

/// Keeps a found violation or a self-test failure as it is (exit status 1)
/// and turns every other error into "could not run" (exit status 2).
fn could_not_run(error: Error) -> Error {
    match error {
        Error::Rules { .. } | Error::SelfTest { .. } | Error::CouldNotRun { .. } => error,
        other => Error::CouldNotRun {
            check: CHECK,
            reason: other.to_string(),
        },
    }
}

/// Violations fail (exit status 1); a run that compared no entry verified
/// nothing (exit status 2).
///
/// # Errors
///
/// Returns the violations, or that nothing was compared.
fn verdict(outcome: &Outcome) -> Result<(), Error> {
    if !outcome.violations.is_empty() {
        return Err(Error::Rules {
            check: CHECK,
            violations: outcome.violations.iter().map(ToString::to_string).collect(),
        });
    }
    if outcome.compared == 0 {
        return Err(Error::CouldNotRun {
            check: CHECK,
            reason: "no inventory entry of this run's target and feature set was compared"
                .to_owned(),
        });
    }
    Ok(())
}

/// Prints the removed and newly quarantined names first, then the
/// quarantined tests skipped and the per-crate counts.
fn report(base: &str, run: &Run, outcome: &Outcome, output: &mut impl Write) -> Result<(), Error> {
    let mut lines = vec![format!(
        "inventory: target {}, features {}, base {base}",
        run.target, run.features
    )];
    for (title, names) in [
        ("removed since the base", &outcome.removed),
        ("newly quarantined", &outcome.newly_quarantined),
        ("skipped while quarantined", &outcome.skipped),
    ] {
        lines.push(format!("inventory: {title}: {}", names.len()));
        lines.extend(names.iter().map(|name| format!("  {name}")));
    }
    lines.extend(
        outcome
            .counts
            .iter()
            .map(|line| format!("inventory: {line}")),
    );
    lines.push(format!(
        "inventory: {} entries compared with the run",
        outcome.compared
    ));
    lines.push(String::new());
    output
        .write_all(lines.join("\n").as_bytes())
        .map_err(Error::io("report inventory"))
}

/// Reads the inventory at `root`, and at `base` through git, reconciles
/// them with `run` and prints the report.
///
/// # Errors
///
/// Returns why the check could not run: an input could not be read, or the
/// run's target is one the inventory does not keep.
fn check_against(
    root: &Path,
    base: &str,
    run: &Run,
    output: &mut impl Write,
) -> Result<Outcome, Error> {
    if !TARGETS.contains(&run.target.as_str()) {
        return Err(Error::CouldNotRun {
            check: CHECK,
            reason: format!(
                "the inventory keeps no entries for target {}; its targets are {}",
                run.target,
                TARGETS.join(", ")
            ),
        });
    }
    let current = source::read_files(&root.join(DIR))?;
    let (shown, base_files) = source::base_files(root, base)?;
    let outcome = reconcile(&current, base_files.as_ref(), run);
    let shown = match &base_files {
        Some(_) => shown,
        None => format!(
            "{shown}, which has no {DIR}: the removal and quarantine rules have nothing to \
             compare against"
        ),
    };
    report(&shown, run, &outcome, output)?;
    Ok(outcome)
}

/// Runs `cargo xtask inventory` in `mode` on this repository.
///
/// # Errors
///
/// Returns the violations (exit status 1), or why it could not run (exit
/// status 2).
pub(super) fn run(mode: InventoryMode, output: &mut impl Write) -> Result<(), Error> {
    run_mode(mode, output).map_err(could_not_run)
}

fn run_mode(mode: InventoryMode, output: &mut impl Write) -> Result<(), Error> {
    let workspace = metadata::load(None, &[])?;
    let root = &workspace.root;
    let cargo = source::Cargo::Repository(root);
    match mode {
        InventoryMode::SelfTest => self_test::run(&workspace.target_dir, output),
        InventoryMode::Update => {
            let run = source::list(&cargo)?;
            let dir = root.join(DIR);
            let current = source::read_files(&dir)?;
            let (changed, untag) = update(&current, &run);
            source::write_files(&dir, &changed)?;
            for (name, _) in &changed {
                writeln!(output, "inventory update: wrote {DIR}/{name}.txt")
                    .map_err(Error::io("report inventory update"))?;
            }
            for name in &untag {
                writeln!(
                    output,
                    "inventory update: {name} runs on {} too; remove the target tag from its \
                     line",
                    run.target
                )
                .map_err(Error::io("report inventory update"))?;
            }
            writeln!(
                output,
                "inventory update: {} files changed for {}; removals are recorded by hand in \
                 {DIR}/{REMOVED}",
                changed.len(),
                run.target
            )
            .map_err(Error::io("report inventory update"))
        }
        InventoryMode::Check { base } => {
            let base = match base {
                Some(base) => base,
                None => source::default_base(root)?,
            };
            let run = source::run(&cargo)?;
            verdict(&check_against(root, &base, &run, output)?)
        }
    }
}
