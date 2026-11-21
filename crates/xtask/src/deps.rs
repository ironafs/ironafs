// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The dependency rules of the architecture (ADR 0004, C4), checked on the
//! resolved graph that `cargo metadata` reports.
//!
//! Every member's manifest inherits the workspace lints and the workspace
//! edition, and every target is in the edition the source checks' token
//! reader follows.

mod pure_source;
mod self_test;

use std::collections::BTreeSet;
use std::fmt;
use std::fs;
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

/// The workspace crates each driver depends on directly. A layer may use
/// `ironafs-os`, so what `ironafs-os` reaches, a layer reaches too.
const DRIVER_MAY_USE: &[(&str, &[&str])] = &[
    // ADR 0005, point 1.4: no dependencies, in any dependency table.
    ("ironafs-os", &[]),
    ("ironafs-net", &["ironafs-rx", "ironafs-time", "ironafs-os"]),
    ("ironafs-gss", &["ironafs-time", "ironafs-os"]),
];

/// For each crate, the third-party crates it may depend on directly.
pub(crate) type ThirdPartyLists = &'static [(&'static str, &'static [&'static str])];

/// The third-party crates each shipped crate may depend on directly. Every
/// list starts empty: a third-party crate is added only after the owner's
/// decision (coding standards rule 21), and a shipped crate missing from
/// this table may use none. A foreign-function crate that `ironafs-gss`
/// uses is named here even when it is reached through another crate.
const THIRD_PARTY: ThirdPartyLists = &[
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
    ("ironafs-client", &[]),
    ("ironafs-net", &[]),
    ("ironafs-gss", &[]),
    ("ironafs-os", &[]),
    ("ironafs-fuse", &[]),
    ("ironafs-smb", &[]),
    ("ironafs-cloudfiles", &[]),
    ("ironafsd", &[]),
    ("ironafs-cli", &[]),
];

/// The only crates through which cryptography and foreign-function crates
/// enter the build (ADR 0004, C4, point 4, and C5).
const CRYPTO_HOMES: &[&str] = &["ironafs-crypto", "ironafs-os"];

/// Crates that may reach the foreign-function crates their third-party
/// list names, and no others: Kerberos and GSS come from each system's own
/// library (ADR 0004, C5, point 4).
const NAMED_FFI: &[&str] = &["ironafs-gss"];

/// Crates whose ADR lets them keep their own `[lints]` table: the `unsafe`
/// bindings of `ironafs-os` (ADR 0005, point 1.5). Each one's table must
/// equal the workspace's except `OWN_LINTS_CHANGE`. A pure crate is never
/// listed: `check-pure` forbids its unsafe code on the command line.
pub(crate) const OWN_LINTS: &[&str] = &["ironafs-os"];

/// Crates whose manifest may hold no dependency table at all, normal,
/// development, build or target-specific (ADR 0005, point 1.4). The
/// resolved graph leaves development dependencies out, so this is read
/// from the manifest itself.
const NO_DEPENDENCY_TABLES: &[&str] = &["ironafs-os"];

/// The edition every target of the workspace is compiled in: the one whose
/// lexical rules the token reader of `rust_source` follows.
const EDITION: &str = "2024";

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
    DriverOutsideList,
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
    /// A crate in `OWN_LINTS` whose lint table is not the workspace's with
    /// `unsafe_code` denied.
    OwnLints {
        name: String,
        manifest: PathBuf,
        differences: Vec<String>,
    },
    /// A crate that may have no dependencies has a dependency table.
    DependencyTable {
        name: String,
        manifest: PathBuf,
        line: String,
    },
    NoWorkspaceLints {
        name: String,
        manifest: PathBuf,
    },
    NoWorkspaceEdition {
        name: String,
        manifest: PathBuf,
    },
    Edition {
        name: String,
        edition: String,
    },
    ForbiddenSource {
        file: PathBuf,
        line: usize,
        name: String,
        reason: &'static str,
    },
    NamedInText {
        file: PathBuf,
        line: usize,
        word: &'static str,
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
            Self::DriverOutsideList => {
                "a driver is thin: of the workspace it depends only on the crates its list in \
                 crates/xtask/src/deps.rs names, so a layer cannot reach the rest through \
                 ironafs-os (ADR 0004, C4, points 3 and 4)"
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
    #[expect(
        clippy::too_many_lines,
        reason = "coding standards rule 10: one arm per violation, each its rule's message"
    )]
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
                 crates/xtask/src/deps.rs does not name: a shipped crate uses a \
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
            Self::OwnLints {
                name,
                manifest,
                differences,
            } => write!(
                formatter,
                "{}: {name}'s own lint table must equal the workspace's except \
                 `unsafe_code = \"deny\"` (ADR 0005, point 1.5): {}",
                manifest.display(),
                differences.join("; ")
            ),
            Self::DependencyTable {
                name,
                manifest,
                line,
            } => write!(
                formatter,
                "{}: {name} has no dependencies in any table (ADR 0005, point 1.4), but its \
                 manifest says `{line}`",
                manifest.display()
            ),
            Self::NoWorkspaceLints { name, manifest } => write!(
                formatter,
                "{}: {name} does not inherit the workspace lints: write `[lints]` with \
                 `workspace = true`, and no multi-line string, so `unsafe` stays forbidden \
                 (AGENTS.md; ADR 0004, C5, point 5)",
                manifest.display()
            ),
            Self::NoWorkspaceEdition { name, manifest } => write!(
                formatter,
                "{}: {name} does not inherit the workspace edition: write \
                 `edition.workspace = true` in `[package]`, name the edition nowhere else, even \
                 in a comment, and write no multi-line string, so the source checks read its \
                 code as the compiler does",
                manifest.display()
            ),
            Self::Edition { name, edition } => write!(
                formatter,
                "{name} has a target in the {edition} edition: every target is in the \
                 {EDITION} edition, whose lexical rules the source checks follow"
            ),
            Self::ForbiddenSource {
                file,
                line,
                name,
                reason,
            } => write!(
                formatter,
                "{}:{line}: names {name}: {reason} (ADR 0004, C4, point 2; coding standards rule 7)",
                file.display()
            ),
            Self::NamedInText { file, line, word } => write!(
                formatter,
                "{}:{line}: says {word}: no file of ironafs-client names a macro that reads \
                 files, even in a comment or a string, so no misread token can hide one \
                 (ADR 0004, C4, point 2)",
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
/// shipped: a shipped crate missing from `lists` may use none. Test-only
/// crates and the automation are never shipped and are not limited.
fn third_party_allowed(
    name: &str,
    kind: Kind,
    lists: ThirdPartyLists,
) -> Option<&'static [&'static str]> {
    (!matches!(kind, Kind::TestOnly | Kind::Automation)).then(|| {
        lists
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
pub(crate) fn graph_violations(
    workspace: &Workspace,
    third_party: ThirdPartyLists,
) -> Vec<Violation> {
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
        violations.extend(direct_violations(workspace, member, from_kind, third_party));
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
            let named = if NAMED_FFI.contains(&member.name.as_str()) {
                third_party_allowed(&member.name, from_kind, third_party).unwrap_or_default()
            } else {
                &[]
            };
            let outside = |found: &Package| !found.member;
            for (to, via) in reachable(workspace, &member.id, &outside) {
                if !to.member
                    && CRYPTO_AND_FFI.contains(&to.name.as_str())
                    && !named.contains(&to.name.as_str())
                {
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
/// crates, a third-party list for every shipped crate, and the workspace
/// crates a layer or a driver may use.
fn direct_violations(
    workspace: &Workspace,
    member: &Member,
    kind: Kind,
    third_party: ThirdPartyLists,
) -> Vec<Violation> {
    let mut violations = Vec::new();
    let dependencies =
        package(workspace, &member.id).map_or(&[][..], |found| found.dependencies.as_slice());
    let driver_list = DRIVER_MAY_USE
        .iter()
        .find_map(|(name, allowed)| (*name == member.name).then_some(*allowed));
    for dependency in dependencies.iter().filter_map(|id| package(workspace, id)) {
        let to = dependency.name.as_str();
        if NO_DEPENDENCIES.contains(&member.name.as_str()) {
            violations.push(Violation::HasDependency {
                name: member.name.clone(),
                dependency: dependency.name.clone(),
            });
        }
        let rule = if !dependency.member {
            if third_party_allowed(&member.name, kind, third_party)
                .is_some_and(|allowed| !allowed.contains(&to))
            {
                violations.push(Violation::ThirdParty {
                    from: member.name.clone(),
                    to: dependency.name.clone(),
                });
            }
            None
        } else if kind == Kind::Layer
            && !LAYER_MAY_USE.contains(&to)
            && !BEHIND_THE_CORE.contains(&to)
        {
            Some(Rule::LayerOutsideList)
        } else if kind == Kind::Driver && !driver_list.unwrap_or_default().contains(&to) {
            Some(Rule::DriverOutsideList)
        } else {
            None
        };
        if let Some(rule) = rule {
            violations.push(Violation::Edge {
                from: member.name.clone(),
                via: Vec::new(),
                to: dependency.name.clone(),
                rule,
            });
        }
    }
    violations
}

/// Every workspace member inherits the workspace lints, which forbid
/// `unsafe` (`AGENTS.md`), unless `own_lints` names it. A crate with its
/// own `[lints]` table, even an empty one, would drop that.
///
/// # Errors
///
/// Returns an error if a manifest cannot be read.
fn lint_violations(workspace: &Workspace, own_lints: &[&str]) -> Result<Vec<Violation>, Error> {
    let mut violations = Vec::new();
    let mut root_manifest: Option<String> = None;
    for member in &workspace.members {
        let (text, manifest) = read_manifest(workspace, member)?;
        if own_lints.contains(&member.name.as_str()) {
            if root_manifest.is_none() {
                let path = workspace.root.join("Cargo.toml");
                let text = fs::read_to_string(&path)
                    .map_err(Error::path_io("read workspace manifest", &path))?;
                root_manifest = Some(text);
            }
            let root = root_manifest.as_deref().unwrap_or_default();
            let differences = own_lints_differences(root, &text);
            if !differences.is_empty() {
                violations.push(Violation::OwnLints {
                    name: member.name.clone(),
                    manifest,
                    differences,
                });
            }
            continue;
        }
        if !inherits_workspace_lints(&text) {
            violations.push(Violation::NoWorkspaceLints {
                name: member.name.clone(),
                manifest,
            });
        }
    }
    Ok(violations)
}

/// The one lint a crate in `OWN_LINTS` sets differently from the
/// workspace, and the level it sets (ADR 0005, point 1.5).
const OWN_LINTS_CHANGE: (&str, &str, &str, &str) =
    ("rust", "unsafe_code", "\"forbid\"", "\"deny\"");

/// The lint entries of a manifest's lint tables, as (group, lint, value)
/// with whitespace and comments dropped: the tables `[<prefix>.<group>]`.
/// `None` if the manifest has a lint table this reader does not read
/// exactly: a multi-line string, a `[<prefix>]` table or a dotted
/// `<prefix>` key, or any other table whose name mentions lints.
fn lint_entries(manifest: &str, prefix: &str) -> Option<BTreeSet<(String, String, String)>> {
    if manifest.contains("\"\"\"") || manifest.contains("'''") {
        return None;
    }
    let group_prefix = format!("{prefix}.");
    let mut table: Option<String> = None;
    let mut entries = BTreeSet::new();
    for line in manifest.lines() {
        let line: String = line
            .split('#')
            .next()
            .unwrap_or_default()
            .chars()
            .filter(|next| !next.is_whitespace())
            .collect();
        if line.is_empty() {
            continue;
        }
        if let Some(header) = line.strip_prefix('[') {
            let name = header.trim_start_matches('[').trim_end_matches(']');
            let group = name.strip_prefix(&group_prefix);
            if group.is_none_or(|group| group.contains('.')) && name.contains("lints") {
                return None;
            }
            table = group.map(str::to_owned);
            continue;
        }
        match &table {
            Some(group) => {
                let (name, value) = line.split_once('=')?;
                entries.insert((group.clone(), name.to_owned(), value.to_owned()));
            }
            None if line.starts_with("lints") => return None,
            None => {}
        }
    }
    Some(entries)
}

/// How a crate's own lint tables depart from the workspace's tables with
/// `OWN_LINTS_CHANGE` applied, one entry per difference: empty when they
/// are equal.
fn own_lints_differences(workspace_manifest: &str, manifest: &str) -> Vec<String> {
    let (group, lint, from, to) = OWN_LINTS_CHANGE;
    let Some(mut expected) = lint_entries(workspace_manifest, "workspace.lints") else {
        return vec!["the workspace's lint tables cannot be read exactly".to_owned()];
    };
    let key = (group.to_owned(), lint.to_owned(), from.to_owned());
    if !expected.remove(&key) {
        return vec![format!(
            "the workspace does not set {group}::{lint} = {from}"
        )];
    }
    expected.insert((group.to_owned(), lint.to_owned(), to.to_owned()));
    let Some(found) = lint_entries(manifest, "lints") else {
        return vec![
            "its lint tables cannot be read exactly: write only `[lints.<group>]` tables, \
             with no multi-line string"
                .to_owned(),
        ];
    };
    if found.is_empty() {
        return vec!["it has no lint table".to_owned()];
    }
    let value_of = |entries: &BTreeSet<(String, String, String)>, group: &str, lint: &str| {
        entries
            .iter()
            .find(|entry| entry.0 == group && entry.1 == lint)
            .map(|entry| entry.2.clone())
    };
    let lints: BTreeSet<(&str, &str)> = expected
        .iter()
        .chain(&found)
        .map(|(group, lint, _)| (group.as_str(), lint.as_str()))
        .collect();
    lints
        .into_iter()
        .filter_map(|(group, lint)| {
            match (
                value_of(&expected, group, lint),
                value_of(&found, group, lint),
            ) {
                (Some(want), Some(have)) if want == have => None,
                (Some(want), Some(have)) => Some(format!("{group}::{lint} is {have}, not {want}")),
                (Some(want), None) => Some(format!("{group}::{lint} = {want} is missing")),
                (None, _) => Some(format!("{group}::{lint} is not in the workspace's table")),
            }
        })
        .collect()
}

/// The members of `names` whose manifest has a dependency table or key.
///
/// # Errors
///
/// Returns an error if a manifest cannot be read.
fn dependency_table_violations(
    workspace: &Workspace,
    names: &[&str],
) -> Result<Vec<Violation>, Error> {
    let mut violations = Vec::new();
    for member in workspace
        .members
        .iter()
        .filter(|member| names.contains(&member.name.as_str()))
    {
        let (text, manifest) = read_manifest(workspace, member)?;
        // Cargo's own reading of the manifest catches every declared
        // dependency, however its key is spelled (an escaped quoted key
        // included); the text check adds empty tables, which cargo does
        // not report.
        let declared = member
            .declared_dependencies
            .first()
            .map(|dependency| format!("declares the {dependency}"));
        if let Some(line) = declared.or_else(|| dependency_line(&text)) {
            violations.push(Violation::DependencyTable {
                name: member.name.clone(),
                manifest,
                line,
            });
        }
    }
    Ok(violations)
}

/// The first line of a manifest, comments aside, that names a dependency
/// table or key (`[dependencies]`, `[dev-dependencies]`,
/// `[build-dependencies]`, `[target.'...'.dependencies]`, or a dotted or
/// inline key), as written.
fn dependency_line(manifest: &str) -> Option<String> {
    manifest
        .lines()
        .map(|line| line.split('#').next().unwrap_or_default().trim())
        .find(|line| line.contains("dependencies"))
        .map(str::to_owned)
}

/// A member's manifest, and its path as shown, relative to the workspace
/// root.
///
/// # Errors
///
/// Returns an error if the manifest cannot be read.
fn read_manifest(workspace: &Workspace, member: &Member) -> Result<(String, PathBuf), Error> {
    let manifest = member.dir.join("Cargo.toml");
    let text =
        fs::read_to_string(&manifest).map_err(Error::path_io("read crate manifest", &manifest))?;
    let shown = manifest
        .strip_prefix(&workspace.root)
        .unwrap_or(&manifest)
        .to_path_buf();
    Ok((text, shown))
}

/// Every workspace member inherits the workspace edition, and every target
/// of it is compiled in `EDITION`, as cargo reports it: the token reader
/// follows that edition's lexical rules, and an older one reads `cr"` and
/// other prefixes otherwise. Each edition other than `EDITION` is reported
/// once per member.
///
/// # Errors
///
/// Returns an error if a manifest cannot be read.
fn edition_violations(workspace: &Workspace) -> Result<Vec<Violation>, Error> {
    let mut violations = Vec::new();
    for member in &workspace.members {
        let (text, manifest) = read_manifest(workspace, member)?;
        if !inherits_workspace_edition(&text) {
            violations.push(Violation::NoWorkspaceEdition {
                name: member.name.clone(),
                manifest,
            });
        }
        let mut reported: Vec<&str> = Vec::new();
        for edition in &member.editions {
            if edition != EDITION && !reported.contains(&edition.as_str()) {
                reported.push(edition);
                violations.push(Violation::Edition {
                    name: member.name.clone(),
                    edition: edition.clone(),
                });
            }
        }
    }
    Ok(violations)
}

/// Whether a manifest says `edition.workspace = true` (or
/// `edition = { workspace = true }`) in its `[package]` table and names
/// the edition on no other line, comments included: a target table can
/// set its own edition. Whitespace is ignored; as for the lints, a
/// manifest with a multi-line string is refused.
fn inherits_workspace_edition(manifest: &str) -> bool {
    if manifest.contains("\"\"\"") || manifest.contains("'''") {
        return false;
    }
    let mut table: Option<String> = None;
    let mut inherits = false;
    for raw in manifest.lines() {
        let line: String = raw
            .split('#')
            .next()
            .unwrap_or_default()
            .chars()
            .filter(|next| !next.is_whitespace())
            .collect();
        if let Some(header) = line.strip_prefix('[') {
            let name = header.trim_start_matches('[').trim_end_matches(']');
            table = Some(name.to_owned());
        }
        if !raw.contains("edition") {
            continue;
        }
        let allowed = line == "edition.workspace=true" || line == "edition={workspace=true}";
        if !allowed || table.as_deref() != Some("package") {
            return false;
        }
        inherits = true;
    }
    inherits
}

/// Whether a manifest's `[lints]` table is exactly `workspace = true`, or
/// it says `lints.workspace = true` or `lints = { workspace = true }` before
/// its first table. Whitespace and comments are ignored; any other spelling
/// is refused rather than guessed at. The manifest is read line by line,
/// so one with a multi-line string, whose lines could look like a table,
/// is refused; so is any table other than `[lints]` whose name mentions
/// lints, such as a target-scoped one.
fn inherits_workspace_lints(manifest: &str) -> bool {
    if manifest.contains("\"\"\"") || manifest.contains("'''") {
        return false;
    }
    let mut table: Option<String> = None;
    let mut inherits = false;
    let mut other_lints = false;
    for line in manifest.lines() {
        let line: String = line
            .split('#')
            .next()
            .unwrap_or_default()
            .chars()
            .filter(|next| !next.is_whitespace())
            .collect();
        if line.is_empty() {
            continue;
        }
        if let Some(header) = line.strip_prefix('[') {
            let name = header.trim_start_matches('[').trim_end_matches(']');
            if name != "lints" && name.contains("lints") {
                other_lints = true;
            }
            table = Some(name.to_owned());
            continue;
        }
        match table.as_deref() {
            None if line == "lints.workspace=true" || line == "lints={workspace=true}" => {
                inherits = true;
            }
            None if line.starts_with("lints") => other_lints = true,
            Some("lints") if line == "workspace=true" => inherits = true,
            Some("lints") => other_lints = true,
            _ => {}
        }
    }
    inherits && !other_lints
}

/// The packages reachable from the package `start`, in breadth-first
/// order, each with the packages between `start` and it on one shortest
/// path. A package `expand` refuses is listed but not followed.
pub(crate) fn reachable<'a>(
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
    let (violations, scanned) = check(&workspace, THIRD_PARTY, NO_DEPENDENCY_TABLES)?;
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
        "check-deps: {} crates passed ({}); {} source files of ironafs-client read",
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
/// Returns an error if a manifest or a source file cannot be read.
fn check(
    workspace: &Workspace,
    third_party: ThirdPartyLists,
    no_tables: &[&str],
) -> Result<(Vec<Violation>, Vec<PathBuf>), Error> {
    let mut violations = graph_violations(workspace, third_party);
    violations.extend(lint_violations(workspace, OWN_LINTS)?);
    violations.extend(dependency_table_violations(workspace, no_tables)?);
    violations.extend(edition_violations(workspace)?);
    let (found, scanned) = pure_source::scan(workspace)?;
    violations.extend(found);
    Ok((violations, scanned))
}
