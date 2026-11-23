// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The workspace's resolved dependency graph, read from `cargo metadata`.

use std::path::{Path, PathBuf};
use std::process::Stdio;

use crate::json::{self, Value};
use crate::{Command, Error};

#[cfg(test)]
mod tests;

/// The workspace members and every package of the resolved graph.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Workspace {
    pub(crate) root: PathBuf,
    pub(crate) target_dir: PathBuf,
    pub(crate) members: Vec<Member>,
    /// Every package the graph resolves, the members included.
    pub(crate) packages: Vec<Package>,
}

/// One workspace member.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Member {
    pub(crate) name: String,
    pub(crate) id: String,
    /// The directory holding the member's manifest.
    pub(crate) dir: PathBuf,
    /// The library target's root file, if it has one.
    pub(crate) lib_root: Option<PathBuf>,
    /// The build script's source file, as Cargo registers it, if the
    /// member has one.
    pub(crate) build_script: Option<PathBuf>,
    /// The kinds of every target, such as `lib`, `bin`, `custom-build` (a
    /// build script) or `test`.
    pub(crate) target_kinds: Vec<String>,
    /// The edition of each target, in the order of `target_kinds`' targets.
    pub(crate) editions: Vec<String>,
    /// Every dependency the manifest declares, as cargo parsed it: normal,
    /// development, build, optional and target-specific alike, each as
    /// `<kind> <name>` (`normal` when cargo gives no kind). Unlike the
    /// resolved graph, this keeps development dependencies.
    pub(crate) declared_dependencies: Vec<String>,
    /// The names the member's source gives dependencies (of every kind)
    /// that are not the dependency's package name with `-` as `_`: a
    /// rename in a manifest, whatever its TOML spelling, or a library
    /// target named otherwise.
    pub(crate) renamed: Vec<String>,
}

impl Member {
    /// The directory holding the library target's root file.
    pub(crate) fn lib_dir(&self) -> Option<&Path> {
        self.lib_root.as_deref().and_then(Path::parent)
    }
}

/// One package of the resolved graph.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Package {
    pub(crate) id: String,
    pub(crate) name: String,
    /// The directory holding the manifest, for a package that comes from a
    /// local path rather than a registry or a git repository.
    pub(crate) path: Option<PathBuf>,
    pub(crate) member: bool,
    /// Package ids of the normal and build dependencies, on any platform
    /// and with every feature on. Development dependencies are left out:
    /// they never reach a shipped build.
    pub(crate) dependencies: Vec<String>,
    /// Package ids reached only as development dependencies, kept apart
    /// for the checks that refuse any edge at all (the canary's).
    pub(crate) dev_dependencies: Vec<String>,
    /// The kinds of every target, as for a member.
    pub(crate) target_kinds: Vec<String>,
    /// The native library the package says it links, if any.
    pub(crate) links: Option<String>,
    /// The names of the features the package declares, implicit ones from
    /// optional dependencies included.
    pub(crate) features: Vec<String>,
}

const COMMAND: &str = "cargo metadata";

/// Runs `cargo metadata` with every feature on and for every platform, and
/// reads the resolved graph. With no manifest it reads this repository,
/// with its lock file held fixed. Each `config` value is passed as a
/// `--config` option.
///
/// # Errors
///
/// Returns an error if cargo fails or its output is not the expected graph.
pub(crate) fn load(manifest: Option<&Path>, config: &[String]) -> Result<Workspace, Error> {
    let mut command = Command::new("cargo");
    command.args(["metadata", "--format-version", "1", "--all-features"]);
    for value in config {
        command.arg("--config").arg(value);
    }
    match manifest {
        Some(manifest) => command
            .args(["--offline", "--quiet"])
            .arg("--manifest-path")
            .arg(manifest),
        None => command.arg("--locked"),
    };
    let output = command
        .stderr(Stdio::inherit())
        .output()
        .map_err(Error::io(COMMAND))?;
    Error::check_status(COMMAND, output.status)?;
    let text = String::from_utf8(output.stdout).map_err(|_| Error::UnexpectedOutput(COMMAND))?;
    parse_workspace(&text)
}

/// A package's declared dependencies, each as `<kind> <name>`.
///
/// # Errors
///
/// Returns an error if the package has no `dependencies` array or a kind
/// that is not a string or null.
fn declared(package: &Value) -> Result<Vec<String>, Error> {
    let mut found = Vec::new();
    for dependency in array(package, "dependencies")? {
        let kind = match dependency.get("kind") {
            None | Some(Value::Null) => "normal",
            Some(Value::String(kind)) => kind.as_str(),
            Some(_) => return Err(Error::UnexpectedOutput(COMMAND)),
        };
        found.push(format!("{kind} {}", string(dependency, "name")?));
    }
    Ok(found)
}

/// What a member's targets say: its library root, its build script, and
/// each target's kinds and edition.
struct Targets {
    lib_root: Option<PathBuf>,
    build_script: Option<PathBuf>,
    kinds: Vec<String>,
    editions: Vec<String>,
}

impl Targets {
    fn read(package: &Value) -> Result<Self, Error> {
        let mut targets = Self {
            lib_root: None,
            build_script: None,
            kinds: Vec::new(),
            editions: Vec::new(),
        };
        for target in array(package, "targets")? {
            targets.editions.push(string(target, "edition")?.to_owned());
            for kind in array(target, "kind")? {
                let kind = kind.as_str().ok_or(Error::UnexpectedOutput(COMMAND))?;
                let source = || string(target, "src_path").map(PathBuf::from);
                match kind {
                    "lib" => targets.lib_root = Some(source()?),
                    "custom-build" => targets.build_script = Some(source()?),
                    _ => {}
                }
                targets.kinds.push(kind.to_owned());
            }
        }
        Ok(targets)
    }
}

/// Reads the workspace from `cargo metadata` output. Dependencies are
/// followed by package id, never by the name a manifest gives them.
///
/// # Errors
///
/// Returns an error if the output is not JSON or lacks the resolved graph.
pub(crate) fn parse_workspace(text: &str) -> Result<Workspace, Error> {
    let unexpected = || Error::UnexpectedOutput(COMMAND);
    let document = json::parse(text).map_err(|_| unexpected())?;
    let packages = array(&document, "packages")?;
    let find_package = |id: &str| {
        packages
            .iter()
            .find(|package| string(package, "id").is_ok_and(|candidate| candidate == id))
            .ok_or_else(unexpected)
    };
    let nodes = document
        .get("resolve")
        .map(|resolve| array(resolve, "nodes"))
        .ok_or_else(unexpected)??;

    let member_ids = array(&document, "workspace_members")?
        .iter()
        .map(|id| id.as_str().ok_or_else(unexpected))
        .collect::<Result<Vec<_>, _>>()?;

    let mut members = Vec::new();
    for id in &member_ids {
        let package = find_package(id)?;
        let manifest = PathBuf::from(string(package, "manifest_path")?);
        let dir = manifest.parent().ok_or_else(unexpected)?.to_path_buf();
        let targets = Targets::read(package)?;
        // Every member must be in the resolved graph. A dependency whose
        // name there is not its package's name is a rename.
        let node = nodes
            .iter()
            .find(|node| string(node, "id").is_ok_and(|candidate| candidate == *id))
            .ok_or_else(unexpected)?;
        members.push(Member {
            name: string(package, "name")?.to_owned(),
            id: (*id).to_owned(),
            dir,
            lib_root: targets.lib_root,
            build_script: targets.build_script,
            target_kinds: targets.kinds,
            editions: targets.editions,
            declared_dependencies: declared(package)?,
            renamed: renamed(node, packages)?,
        });
    }

    let mut resolved = Vec::new();
    for node in nodes {
        let id = string(node, "id")?;
        let package = find_package(id)?;
        let path = match package.get("source") {
            None | Some(Value::Null) => {
                let manifest = PathBuf::from(string(package, "manifest_path")?);
                Some(manifest.parent().ok_or_else(unexpected)?.to_path_buf())
            }
            Some(Value::String(_)) => None,
            Some(_) => return Err(unexpected()),
        };
        let mut dependencies = Vec::new();
        let mut dev_dependencies = Vec::new();
        for dependency in array(node, "deps")? {
            let mut shipped = false;
            for kind in array(dependency, "dep_kinds")? {
                match kind.get("kind") {
                    Some(Value::Null) => shipped = true,
                    Some(Value::String(kind)) if kind == "build" => shipped = true,
                    Some(Value::String(kind)) if kind == "dev" => {}
                    _ => return Err(unexpected()),
                }
            }
            let target = string(dependency, "pkg")?;
            find_package(target)?;
            if shipped {
                dependencies.push(target.to_owned());
            } else {
                dev_dependencies.push(target.to_owned());
            }
        }
        resolved.push(Package {
            id: id.to_owned(),
            name: string(package, "name")?.to_owned(),
            path,
            member: member_ids.contains(&id),
            dependencies,
            dev_dependencies,
            target_kinds: target_kinds(package)?,
            links: links(package)?,
            features: features(package)?,
        });
    }
    Ok(Workspace {
        root: PathBuf::from(string(&document, "workspace_root")?),
        target_dir: PathBuf::from(string(&document, "target_directory")?),
        members,
        packages: resolved,
    })
}

/// The names a resolved node gives its dependencies that are not their
/// package's name with `-` as `_`.
fn renamed(node: &Value, packages: &[Value]) -> Result<Vec<String>, Error> {
    let mut renamed = Vec::new();
    for dependency in array(node, "deps")? {
        let name = string(dependency, "name")?;
        let id = string(dependency, "pkg")?;
        let package = packages
            .iter()
            .find(|package| string(package, "id").is_ok_and(|candidate| candidate == id))
            .ok_or(Error::UnexpectedOutput(COMMAND))?;
        if name != string(package, "name")?.replace('-', "_") {
            renamed.push(name.to_owned());
        }
    }
    Ok(renamed)
}

/// The kinds of a package's targets.
fn target_kinds(package: &Value) -> Result<Vec<String>, Error> {
    let mut kinds = Vec::new();
    for target in array(package, "targets")? {
        for kind in array(target, "kind")? {
            kinds.push(
                kind.as_str()
                    .ok_or(Error::UnexpectedOutput(COMMAND))?
                    .to_owned(),
            );
        }
    }
    Ok(kinds)
}

/// The native library a package links, if any.
fn links(package: &Value) -> Result<Option<String>, Error> {
    match package.get("links") {
        Some(Value::Null) => Ok(None),
        Some(Value::String(links)) => Ok(Some(links.clone())),
        _ => Err(Error::UnexpectedOutput(COMMAND)),
    }
}

/// The names of the features a package declares.
fn features(package: &Value) -> Result<Vec<String>, Error> {
    match package.get("features") {
        Some(Value::Object(features)) => {
            Ok(features.iter().map(|(name, _)| name.clone()).collect())
        }
        _ => Err(Error::UnexpectedOutput(COMMAND)),
    }
}

fn array<'a>(value: &'a Value, key: &str) -> Result<&'a [Value], Error> {
    value
        .get(key)
        .and_then(Value::as_array)
        .ok_or(Error::UnexpectedOutput(COMMAND))
}

fn string<'a>(value: &'a Value, key: &str) -> Result<&'a str, Error> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or(Error::UnexpectedOutput(COMMAND))
}
