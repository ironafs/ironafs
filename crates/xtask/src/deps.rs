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
use crate::metadata::{self, Member, Package, Workspace};

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
/// `ironafs-events` is not one: Rx reports through its own observer or
/// typed outcomes.
const RX_MAY_USE: &[&str] = &["ironafs-xdr", "ironafs-time", "ironafs-crypto"];

/// What a layer crate never reaches (ADR 0004, C4, point 4).
const BEHIND_THE_CORE: &[&str] = &["ironafs-client", "ironafs-cache"];

/// The only workspace crates a layer crate depends on directly (ADR 0004,
/// C4, point 4): the contract, events, IPC, cryptography, time, and its
/// own operating-system bindings.
const LAYER_MAY_USE: &[&str] = &[
    "ironafs-vfs",
    "ironafs-events",
    "ironafs-ipc",
    "ironafs-crypto",
    "ironafs-time",
    "ironafs-os",
];

/// The crates with no dependencies at all (ADR 0004, C4, point 1, and O1,
/// point 1).
const NO_DEPENDENCIES: &[&str] = &["ironafs-time", "ironafs-events"];

/// The third-party crates each pure and layer crate may depend on
/// directly. Every list starts empty: a third-party crate is added only
/// after the owner's decision (coding standards rule 21), and a crate
/// missing from this table may use none.
const THIRD_PARTY: &[(&str, &[&str])] = &[
    ("ironafs-xdr", &[]),
    ("ironafs-rx", &[]),
    ("ironafs-rxkad", &[]),
    ("ironafs-rxgk", &[]),
    ("ironafs-proto", &[]),
    ("ironafs-vfs", &[]),
    ("ironafs-cache", &[]),
    ("ironafs-auth", &[]),
    ("ironafs-ipc", &[]),
    ("ironafs-config", &[]),
    ("ironafs-events", &[]),
    ("ironafs-crypto", &[]),
    ("ironafs-time", &[]),
    ("ironafs-fuse", &[]),
    ("ironafs-smb", &[]),
    ("ironafs-cloudfiles", &[]),
];

/// The only crates through which cryptography and foreign-function crates
/// enter the build (ADR 0004, C4, point 4, and C5).
const CRYPTO_HOMES: &[&str] = &["ironafs-crypto", "ironafs-os"];

/// Known cryptography and foreign-function crates, which only the crates
/// in `CRYPTO_HOMES` may reach.
const CRYPTO_AND_FFI: &[&str] = &[
    "aes",
    "aes-gcm",
    "aws-lc-rs",
    "aws-lc-sys",
    "blake2",
    "blake3",
    "boring",
    "boring-sys",
    "camellia",
    "cbc",
    "ccm",
    "chacha20",
    "chacha20poly1305",
    "cipher",
    "cmac",
    "core-foundation",
    "core-foundation-sys",
    "ctr",
    "curve25519-dalek",
    "des",
    "digest",
    "ed25519-dalek",
    "getrandom",
    "hkdf",
    "hmac",
    "k256",
    "libc",
    "libgssapi",
    "libloading",
    "libsodium-sys",
    "mbedtls",
    "md-5",
    "md5",
    "nettle",
    "nix",
    "objc2",
    "openssl",
    "openssl-sys",
    "p256",
    "pbkdf2",
    "rand",
    "rand_core",
    "ring",
    "rsa",
    "rustix",
    "security-framework",
    "security-framework-sys",
    "sha1",
    "sha2",
    "sha3",
    "sodiumoxide",
    "winapi",
    "windows",
    "windows-sys",
    "x25519-dalek",
];

/// A broken edge rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Rule {
    PureOnNonPure,
    LayerBypassesCore,
    LayerOutsideList,
    RxKnowsAfs,
    ProtocolOnClientSide,
    ShippedOnTestOnly,
    CoreOnDriver,
    OnAutomation,
}

/// One broken rule.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Violation {
    EmptyWorkspace,
    Unclassified(String),
    NotAMember {
        name: String,
        dir: PathBuf,
    },
    OutsideCrates {
        name: String,
        dir: PathBuf,
    },
    Edge {
        from: String,
        via: Vec<String>,
        to: String,
        rule: Rule,
    },
    HasDependency {
        name: String,
        dependency: String,
    },
    ThirdParty {
        from: String,
        to: String,
    },
    CryptoOutsideHome {
        from: String,
        via: Vec<String>,
        to: String,
    },
    ShippedTarget {
        name: String,
        target: String,
    },
    ForbiddenSource {
        file: PathBuf,
        line: usize,
        name: String,
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
            Self::LayerOutsideList => {
                "of the workspace a layer crate depends only on ironafs-vfs, ironafs-events, \
                 ironafs-ipc, ironafs-crypto, ironafs-time and its operating-system bindings, \
                 ironafs-os (ADR 0004, C4, point 4)"
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
            Self::CoreOnDriver => {
                "ironafs-client depends on no driver, layer or binary crate: it makes no socket \
                 or operating-system call (ADR 0004, C4, point 2)"
            }
            Self::OnAutomation => {
                "only the repository's automation depends on xtask, which is never shipped \
                 (ADR 0004, C4, point 3)"
            }
        }
    }
}

fn through(formatter: &mut fmt::Formatter<'_>, via: &[String]) -> fmt::Result {
    if via.is_empty() {
        Ok(())
    } else {
        write!(formatter, " through {}", via.join(" -> "))
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
            Self::NotAMember { name, dir } => write!(
                formatter,
                "{name} at {} is a local crate outside the workspace: every local crate is a \
                 workspace member, so these rules classify it (ADR 0004, C4)",
                dir.display()
            ),
            Self::OutsideCrates { name, dir } => write!(
                formatter,
                "{name} at {} is a workspace crate outside crates/ (coding standards rule 20)",
                dir.display()
            ),
            Self::Edge {
                from,
                via,
                to,
                rule,
            } => {
                write!(formatter, "{from} depends on {to}")?;
                through(formatter, via)?;
                write!(formatter, ": {}", rule.text())
            }
            Self::HasDependency { name, dependency } => write!(
                formatter,
                "{name} depends on {dependency}: it has no dependencies \
                 (ADR 0004, C4, point 1, and O1, point 1)"
            ),
            Self::ThirdParty { from, to } => write!(
                formatter,
                "{from} depends on the third-party crate {to}, which its list in \
                 crates/xtask/src/deps.rs does not name: a pure or layer crate uses a \
                 third-party crate only after the owner's decision (coding standards rule 21)"
            ),
            Self::CryptoOutsideHome { from, via, to } => {
                write!(formatter, "{from} depends on {to}")?;
                through(formatter, via)?;
                write!(
                    formatter,
                    ": cryptography and foreign-function crates enter only through \
                     ironafs-crypto and ironafs-os (ADR 0004, C4, point 4, and C5)"
                )
            }
            Self::ShippedTarget { name, target } => write!(
                formatter,
                "{name} has a {target} target: a pure crate and ironafs-client ship only their \
                 library, and a build script would run I/O at build time (ADR 0004, C3, point 1)"
            ),
            Self::ForbiddenSource {
                file,
                line,
                name,
                reason,
            } => write!(
                formatter,
                "{}:{line}: names {name}: {reason} (ADR 0004, C3, point 1, and C4, points 1 and 2)",
                file.display()
            ),
            Self::NoLibrary(name) => write!(
                formatter,
                "{name} is a crate whose source is checked, with no library target"
            ),
            Self::NoSource(name) => write!(
                formatter,
                "{name} is a crate whose source is checked, with no library source file"
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

/// The third-party crates a crate may depend on directly, if the crate is
/// limited.
fn third_party_allowed(name: &str, kind: Kind) -> Option<&'static [&'static str]> {
    matches!(kind, Kind::Pure | Kind::Layer).then(|| {
        THIRD_PARTY
            .iter()
            .find_map(|(candidate, allowed)| (*candidate == name).then_some(*allowed))
            .unwrap_or_default()
    })
}

fn broken_rule(from: &str, from_kind: Kind, to: &str, to_kind: Kind) -> Option<Rule> {
    if to_kind == Kind::TestOnly && !matches!(from_kind, Kind::TestOnly | Kind::Automation) {
        Some(Rule::ShippedOnTestOnly)
    } else if to_kind == Kind::Automation && from_kind != Kind::Automation {
        Some(Rule::OnAutomation)
    } else if from_kind == Kind::Pure && to_kind != Kind::Pure {
        Some(Rule::PureOnNonPure)
    } else if from_kind == Kind::Composition
        && matches!(to_kind, Kind::Driver | Kind::Layer | Kind::Binary)
    {
        Some(Rule::CoreOnDriver)
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

fn package<'a>(workspace: &'a Workspace, id: &str) -> Option<&'a Package> {
    workspace.packages.iter().find(|package| package.id == id)
}

/// Checks the crate table and every rule against each crate's transitive
/// normal and build dependencies, followed through every package of the
/// resolved graph, workspace member or not.
pub(crate) fn graph_violations(workspace: &Workspace) -> Vec<Violation> {
    if workspace.members.is_empty() {
        return vec![Violation::EmptyWorkspace];
    }
    let mut violations = Vec::new();
    let crates_dir = workspace.root.join("crates");
    for found in &workspace.packages {
        let Some(dir) = &found.path else {
            continue;
        };
        if !found.member {
            violations.push(Violation::NotAMember {
                name: found.name.clone(),
                dir: dir.clone(),
            });
        } else if dir.parent() != Some(crates_dir.as_path()) {
            violations.push(Violation::OutsideCrates {
                name: found.name.clone(),
                dir: dir.clone(),
            });
        }
    }
    for member in &workspace.members {
        if kind_of(&member.name).is_none() {
            violations.push(Violation::Unclassified(member.name.clone()));
        }
    }
    for member in &workspace.members {
        let Some(from_kind) = kind_of(&member.name) else {
            continue;
        };
        violations.extend(direct_violations(workspace, member, from_kind));
        for (to, via) in reachable(workspace, &member.id, &|_| true) {
            let Some(to_kind) = kind_of(&to.name).filter(|_| to.member) else {
                continue;
            };
            if let Some(rule) = broken_rule(&member.name, from_kind, &to.name, to_kind) {
                violations.push(Violation::Edge {
                    from: member.name.clone(),
                    via,
                    to: to.name.clone(),
                    rule,
                });
            }
        }
        if !CRYPTO_HOMES.contains(&member.name.as_str()) {
            // Through third-party crates only: a workspace crate on the way
            // is checked as the start of its own walk, and the homes may
            // use these crates.
            let third_party = |found: &Package| !found.member;
            for (to, via) in reachable(workspace, &member.id, &third_party) {
                if !to.member && CRYPTO_AND_FFI.contains(&to.name.as_str()) {
                    violations.push(Violation::CryptoOutsideHome {
                        from: member.name.clone(),
                        via,
                        to: to.name.clone(),
                    });
                }
            }
        }
    }
    violations
}

/// The rules on a member's direct dependencies: none at all for some
/// crates, a third-party list for pure and layer crates, and the workspace
/// crates a layer may use.
fn direct_violations(workspace: &Workspace, member: &Member, kind: Kind) -> Vec<Violation> {
    let mut violations = Vec::new();
    let dependencies =
        package(workspace, &member.id).map_or(&[][..], |found| found.dependencies.as_slice());
    for dependency in dependencies.iter().filter_map(|id| package(workspace, id)) {
        if NO_DEPENDENCIES.contains(&member.name.as_str()) {
            violations.push(Violation::HasDependency {
                name: member.name.clone(),
                dependency: dependency.name.clone(),
            });
        }
        if !dependency.member {
            if third_party_allowed(&member.name, kind)
                .is_some_and(|allowed| !allowed.contains(&dependency.name.as_str()))
            {
                violations.push(Violation::ThirdParty {
                    from: member.name.clone(),
                    to: dependency.name.clone(),
                });
            }
        } else if kind == Kind::Layer
            && !LAYER_MAY_USE.contains(&dependency.name.as_str())
            && !BEHIND_THE_CORE.contains(&dependency.name.as_str())
        {
            violations.push(Violation::Edge {
                from: member.name.clone(),
                via: Vec::new(),
                to: dependency.name.clone(),
                rule: Rule::LayerOutsideList,
            });
        }
    }
    violations
}

/// The packages reachable from the package `start`, in breadth-first
/// order, each with the packages between `start` and it on one shortest
/// path. A package `expand` refuses is listed but not followed.
fn reachable<'a>(
    workspace: &'a Workspace,
    start: &str,
    expand: &dyn Fn(&Package) -> bool,
) -> Vec<(&'a Package, Vec<String>)> {
    let mut found: Vec<(&Package, Vec<String>)> = Vec::new();
    let mut next = 0;
    let mut frontier: Vec<(String, Vec<String>)> = vec![(start.to_owned(), Vec::new())];
    while let Some((id, path)) = frontier.get(next).cloned() {
        next += 1;
        let Some(current) = package(workspace, &id) else {
            continue;
        };
        if id != start && !expand(current) {
            continue;
        }
        for dependency in current
            .dependencies
            .iter()
            .filter_map(|id| package(workspace, id))
        {
            let seen =
                dependency.id == start || found.iter().any(|(seen, _)| seen.id == dependency.id);
            if seen {
                continue;
            }
            let mut via = path.clone();
            if id != start {
                via.push(current.name.clone());
            }
            found.push((dependency, via.clone()));
            frontier.push((dependency.id.clone(), via));
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
    let workspace = metadata::load(None, &[])?;
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
        "check-deps: {} crates passed ({}); {} source files of pure crates and ironafs-client read",
        names.len(),
        names.join(", "),
        scanned.len()
    )
    .map_err(Error::io("report check-deps"))
}

/// Runs every rule on a workspace; returns the violations and the source
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
