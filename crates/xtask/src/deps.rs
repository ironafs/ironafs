// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The dependency rules of the architecture (ADR 0004, C4), checked on the
//! resolved graph that `cargo metadata` reports.

mod pure_source;
mod self_test;

use std::fmt;
use std::io::Write;
use std::path::PathBuf;

use crate::Error;
use crate::metadata::{self, Workspace};

#[cfg(test)]
mod tests;

/// What a crate is, which decides what it may depend on (ADR 0004, C4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    /// No I/O and no `unsafe` (point 1).
    Pure,
    /// `ironafs-client`, the core (point 2).
    Composition,
    /// A driver or operating-system binding (point 3).
    Driver,
    /// An operating-system layer: a driver that serves files (point 4).
    Layer,
    /// A shipped binary (point 3).
    Binary,
    /// Repository automation, never shipped (point 3).
    Automation,
    /// Test-only, never shipped (point 7).
    TestOnly,
}

/// Every crate the architecture names. A new workspace crate must be added
/// here before the check passes.
pub(crate) const CRATES: &[(&str, Kind)] = &[
    ("ironafs-xdr", Kind::Pure),
    ("ironafs-rx", Kind::Pure),
    ("ironafs-rxkad", Kind::Pure),
    ("ironafs-rxgk", Kind::Pure),
    ("ironafs-proto", Kind::Pure),
    ("ironafs-vfs", Kind::Pure),
    ("ironafs-cache", Kind::Pure),
    ("ironafs-auth", Kind::Pure),
    ("ironafs-ipc", Kind::Pure),
    ("ironafs-config", Kind::Pure),
    ("ironafs-events", Kind::Pure),
    ("ironafs-crypto", Kind::Pure),
    ("ironafs-time", Kind::Pure),
    ("ironafs-client", Kind::Composition),
    ("ironafs-net", Kind::Driver),
    ("ironafs-gss", Kind::Driver),
    ("ironafs-os", Kind::Driver),
    ("ironafs-fuse", Kind::Layer),
    ("ironafs-smb", Kind::Layer),
    ("ironafs-cloudfiles", Kind::Layer),
    ("ironafsd", Kind::Binary),
    ("ironafs-cli", Kind::Binary),
    ("xtask", Kind::Automation),
    ("ironafs-sim", Kind::TestOnly),
    ("ironafs-test-support", Kind::TestOnly),
];

/// The protocol crates (ADR 0004, C4, point 1).
const PROTOCOL: &[&str] = &[
    "ironafs-xdr",
    "ironafs-rx",
    "ironafs-rxkad",
    "ironafs-rxgk",
    "ironafs-proto",
];

/// The crates that hold the client's own model of files: the core, its
/// cache state and the contract with the layers.
const CLIENT_SIDE: &[&str] = &["ironafs-client", "ironafs-cache", "ironafs-vfs"];

/// The only workspace crates `ironafs-rx` may use: none of them knows AFS.
const RX_MAY_USE: &[&str] = &["ironafs-xdr", "ironafs-time", "ironafs-crypto"];

/// What a layer crate never reaches (ADR 0004, C4, point 4).
const BEHIND_THE_CORE: &[&str] = &["ironafs-client", "ironafs-cache"];

/// The crate with no dependencies at all (ADR 0004, C4, point 1).
const NO_DEPENDENCIES: &str = "ironafs-time";

/// A broken edge rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Rule {
    PureOnNonPure,
    LayerBypassesCore,
    RxKnowsAfs,
    ProtocolOnClientSide,
    ShippedOnTestOnly,
}

/// One broken rule.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Violation {
    EmptyWorkspace,
    Unclassified(String),
    Edge {
        from: String,
        via: Vec<String>,
        to: String,
        rule: Rule,
    },
    TimeHasDependency(String),
    ForbiddenStdPath {
        file: PathBuf,
        line: usize,
        path: String,
        reason: &'static str,
    },
    NoLibrary(String),
    NoSource(String),
    Unreadable {
        file: PathBuf,
        error: String,
    },
}

impl Rule {
    fn text(self) -> &'static str {
        match self {
            Self::PureOnNonPure => {
                "a pure crate depends only on pure crates (ADR 0004, C4, points 1 and 4)"
            }
            Self::LayerBypassesCore => {
                "no layer crate depends on ironafs-client or ironafs-cache (ADR 0004, C4, point 4)"
            }
            Self::RxKnowsAfs => {
                "ironafs-rx knows nothing of AFS: of the workspace it uses only ironafs-xdr, \
                 ironafs-time and ironafs-crypto (ADR 0004, C4, point 4)"
            }
            Self::ProtocolOnClientSide => {
                "protocol crates depend on no client-side crate (ironafs-client, ironafs-cache, \
                 ironafs-vfs) (ADR 0004, C4, point 4)"
            }
            Self::ShippedOnTestOnly => {
                "no shipped crate depends on a test-only crate, except as a development \
                 dependency (ADR 0004, C4, point 7)"
            }
        }
    }
}

impl fmt::Display for Violation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyWorkspace => write!(
                formatter,
                "cargo metadata lists no workspace crates, so nothing was checked"
            ),
            Self::Unclassified(name) => write!(
                formatter,
                "{name} is not in the crate table in crates/xtask/src/deps.rs: \
                 classify it under ADR 0004, C4"
            ),
            Self::Edge {
                from,
                via,
                to,
                rule,
            } => {
                write!(formatter, "{from} depends on {to}")?;
                if !via.is_empty() {
                    write!(formatter, " through {}", via.join(" -> "))?;
                }
                write!(formatter, ": {}", rule.text())
            }
            Self::TimeHasDependency(dependency) => write!(
                formatter,
                "{NO_DEPENDENCIES} depends on {dependency}: it has no dependencies \
                 (ADR 0004, C4, point 1)"
            ),
            Self::ForbiddenStdPath {
                file,
                line,
                path,
                reason,
            } => write!(
                formatter,
                "{}:{line}: a pure crate names {path}: {reason} (ADR 0004, C3, point 1)",
                file.display()
            ),
            Self::NoLibrary(name) => write!(
                formatter,
                "{name} is a pure crate with no library target to check"
            ),
            Self::NoSource(name) => write!(
                formatter,
                "{name} is a pure crate with no library source file to check"
            ),
            Self::Unreadable { file, error } => {
                write!(
                    formatter,
                    "{}: cannot read as Rust: {error}",
                    file.display()
                )
            }
        }
    }
}

/// The kind the crate table gives a crate.
pub(crate) fn kind_of(name: &str) -> Option<Kind> {
    CRATES
        .iter()
        .find_map(|(candidate, kind)| (*candidate == name).then_some(*kind))
}

fn broken_rule(from: &str, from_kind: Kind, to: &str, to_kind: Kind) -> Option<Rule> {
    if to_kind == Kind::TestOnly && !matches!(from_kind, Kind::TestOnly | Kind::Automation) {
        Some(Rule::ShippedOnTestOnly)
    } else if from_kind == Kind::Pure && to_kind != Kind::Pure {
        Some(Rule::PureOnNonPure)
    } else if from_kind == Kind::Layer && BEHIND_THE_CORE.contains(&to) {
        Some(Rule::LayerBypassesCore)
    } else if from == "ironafs-rx" && !RX_MAY_USE.contains(&to) {
        Some(Rule::RxKnowsAfs)
    } else if PROTOCOL.contains(&from) && CLIENT_SIDE.contains(&to) {
        Some(Rule::ProtocolOnClientSide)
    } else {
        None
    }
}

/// Checks the crate table and every rule against each crate's transitive
/// normal and build dependencies.
pub(crate) fn graph_violations(workspace: &Workspace) -> Vec<Violation> {
    if workspace.members.is_empty() {
        return vec![Violation::EmptyWorkspace];
    }
    let mut violations = Vec::new();
    for member in &workspace.members {
        if kind_of(&member.name).is_none() {
            violations.push(Violation::Unclassified(member.name.clone()));
        }
        if member.name == NO_DEPENDENCIES {
            violations.extend(
                member
                    .dependencies
                    .iter()
                    .map(|dependency| Violation::TimeHasDependency(dependency.clone())),
            );
        }
    }
    for member in &workspace.members {
        let Some(from_kind) = kind_of(&member.name) else {
            continue;
        };
        for (to, via) in reachable(workspace, &member.name) {
            let Some(to_kind) = kind_of(&to) else {
                continue;
            };
            if let Some(rule) = broken_rule(&member.name, from_kind, &to, to_kind) {
                violations.push(Violation::Edge {
                    from: member.name.clone(),
                    via,
                    to,
                    rule,
                });
            }
        }
    }
    violations
}

/// The workspace crates reachable from `start`, in breadth-first order, each
/// with the crates between `start` and it on one shortest path.
fn reachable(workspace: &Workspace, start: &str) -> Vec<(String, Vec<String>)> {
    let dependencies = |name: &str| {
        workspace
            .members
            .iter()
            .find(|member| member.name == name)
            .map_or(&[][..], |member| member.dependencies.as_slice())
    };
    let mut found: Vec<(String, Vec<String>)> = Vec::new();
    let mut next = 0;
    let mut frontier = vec![(start.to_owned(), Vec::new())];
    while let Some((name, path)) = frontier.get(next).cloned() {
        next += 1;
        for dependency in dependencies(&name) {
            let is_member = workspace
                .members
                .iter()
                .any(|member| &member.name == dependency);
            let seen = dependency == start || found.iter().any(|(seen, _)| seen == dependency);
            if !is_member || seen {
                continue;
            }
            let mut via = path.clone();
            if name != start {
                via.push(name.clone());
            }
            found.push((dependency.clone(), via.clone()));
            frontier.push((dependency.clone(), via));
        }
    }
    found
}

/// Checks this repository's workspace, or with `self_test` proves on
/// planted fixtures that each rule is caught.
///
/// # Errors
///
/// Returns the violations found, or why the check could not run.
pub(super) fn run(self_test: bool, output: &mut impl Write) -> Result<(), Error> {
    let workspace = metadata::load(None)?;
    if self_test {
        return self_test::run(&workspace.target_dir, output);
    }
    let (violations, scanned) = check(&workspace)?;
    if !violations.is_empty() {
        return Err(Error::Rules {
            check: "check-deps",
            violations: violations.iter().map(ToString::to_string).collect(),
        });
    }
    let names: Vec<&str> = workspace
        .members
        .iter()
        .map(|member| member.name.as_str())
        .collect();
    writeln!(
        output,
        "check-deps: {} crates passed ({}); {} pure-crate source files read",
        names.len(),
        names.join(", "),
        scanned.len()
    )
    .map_err(Error::io("report check-deps"))
}

/// Runs every rule on a workspace; returns the violations and the pure-crate
/// files read.
///
/// # Errors
///
/// Returns an error if a source file cannot be read.
fn check(workspace: &Workspace) -> Result<(Vec<Violation>, Vec<PathBuf>), Error> {
    let mut violations = graph_violations(workspace);
    let (found, scanned) = pure_source::scan(workspace)?;
    violations.extend(found);
    Ok((violations, scanned))
}
