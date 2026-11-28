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
    pub(crate) targets: Vec<Target>,
    /// The kinds of every target, such as `lib`, `bin`, `custom-build` (a
    /// build script) or `test`.
    pub(crate) target_kinds: Vec<String>,
    /// The edition of each target, in the order of `target_kinds`' targets.
    pub(crate) editions: Vec<String>,
    /// Every dependency the manifest declares, as cargo parsed it: normal,
    /// development, build, optional and target-specific alike. Unlike the
    /// resolved graph, this keeps development dependencies, and what each
    /// declaration asks of its dependency.
    pub(crate) declarations: Vec<Declaration>,
    /// The names the member's source gives dependencies (of every kind)
    /// that are not the dependency's package name with `-` as `_`: a
    /// rename in a manifest, whatever its TOML spelling, or a library
    /// target named otherwise.
    pub(crate) renamed: Vec<String>,
}

/// One dependency a member's manifest declares.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Declaration {
    /// `normal`, `dev` or `build` (`normal` when cargo gives no kind).
    pub(crate) kind: String,
    /// The dependency's package name.
    pub(crate) package: String,
    /// The name the manifest gives it: its rename, or the package name.
    pub(crate) name: String,
    /// Whether the manifest renames it, a rename to its own package name
    /// included.
    pub(crate) renamed: bool,
    /// The platform it is declared for, as Cargo writes it.
    pub(crate) target: Option<String>,
    pub(crate) optional: bool,
    /// The features the declaration asks of the dependency.
    pub(crate) features: Vec<String>,
}

impl std::fmt::Display for Declaration {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{} {}", self.kind, self.package)
    }
}

/// One edge of the resolved graph: the name the dependent's code gives the
/// dependency, the package it resolved to, and each kind (`normal`, `dev`
/// or `build`) and platform it is a dependency of.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Edge {
    pub(crate) name: String,
    pub(crate) package: String,
    pub(crate) kinds: Vec<(String, Option<String>)>,
}

/// One Cargo target bearing Rust source.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Target {
    pub(crate) name: String,
    pub(crate) kinds: Vec<String>,
    pub(crate) root: PathBuf,
    pub(crate) required_features: Vec<String>,
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
    /// The package's features, each with what it enables, as its manifest
    /// states them (implicit features of optional dependencies included).
    pub(crate) features: Vec<(String, Vec<String>)>,
    /// Every dependency its manifest declares, as for a member.
    pub(crate) declarations: Vec<Declaration>,
    /// The package's edges in the resolved graph, of every kind.
    pub(crate) edges: Vec<Edge>,
    /// The name of its library target, which names it in the code of a
    /// dependent that does not rename it.
    pub(crate) lib_name: Option<String>,
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
    load_in(manifest, config, |_| {})
}

/// As `load`, with `prepare` applied to the `cargo metadata` command first
/// (to set the compiler it runs, for one).
///
/// # Errors
///
/// Returns an error if cargo fails or its output is not the expected graph.
pub(crate) fn load_in(
    manifest: Option<&Path>,
    config: &[String],
    prepare: impl FnOnce(&mut Command),
) -> Result<Workspace, Error> {
    let mut command = Command::new("cargo");
    prepare(&mut command);
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

/// A package's declared dependencies.
///
/// # Errors
///
/// Returns an error if the package has no `dependencies` array, or a
/// declaration's kind, rename, optional flag or features are not what
/// cargo writes.
fn declared(package: &Value) -> Result<Vec<Declaration>, Error> {
    let mut found = Vec::new();
    for dependency in array(package, "dependencies")? {
        let kind = match dependency.get("kind") {
            None | Some(Value::Null) => "normal",
            Some(Value::String(kind)) => kind.as_str(),
            Some(_) => return Err(Error::UnexpectedOutput(COMMAND)),
        };
        let package = string(dependency, "name")?;
        let (name, renamed) = match dependency.get("rename") {
            Some(Value::Null) => (package, false),
            Some(Value::String(rename)) => (rename.as_str(), true),
            _ => return Err(Error::UnexpectedOutput(COMMAND)),
        };
        let target = match dependency.get("target") {
            Some(Value::Null) => None,
            Some(Value::String(target)) => Some(target.clone()),
            _ => return Err(Error::UnexpectedOutput(COMMAND)),
        };
        let Some(Value::Bool(optional)) = dependency.get("optional") else {
            return Err(Error::UnexpectedOutput(COMMAND));
        };
        found.push(Declaration {
            kind: kind.to_owned(),
            package: package.to_owned(),
            name: name.to_owned(),
            renamed,
            target,
            optional: *optional,
            features: strings(array(dependency, "features")?)?,
        });
    }
    Ok(found)
}

/// A JSON array of strings.
fn strings(values: &[Value]) -> Result<Vec<String>, Error> {
    values
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or(Error::UnexpectedOutput(COMMAND))
        })
        .collect()
}

/// What a member's targets say: its library root, its build script, and
/// each target's kinds and edition.
struct Targets {
    lib_root: Option<PathBuf>,
    build_script: Option<PathBuf>,
    kinds: Vec<String>,
    editions: Vec<String>,
    sources: Vec<Target>,
}

impl Targets {
    fn read(package: &Value) -> Result<Self, Error> {
        let mut targets = Self {
            lib_root: None,
            build_script: None,
            kinds: Vec::new(),
            editions: Vec::new(),
            sources: Vec::new(),
        };
        for target in array(package, "targets")? {
            targets.editions.push(string(target, "edition")?.to_owned());
            let source = PathBuf::from(string(target, "src_path")?);
            let kinds = kinds_of(target)?;
            let required_features = match target.get("required-features") {
                None => Vec::new(),
                Some(_) => strings(array(target, "required-features")?)?,
            };
            targets.sources.push(Target {
                name: string(target, "name")?.to_owned(),
                kinds: kinds.iter().map(|kind| (*kind).to_owned()).collect(),
                root: source.clone(),
                required_features,
            });
            for kind in kinds {
                match kind {
                    "lib" => targets.lib_root = Some(source.clone()),
                    "custom-build" => targets.build_script = Some(source.clone()),
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
            targets: targets.sources,
            target_kinds: targets.kinds,
            editions: targets.editions,
            declarations: declared(package)?,
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
        let (dependencies, dev_dependencies, edges) = node_edges(node, &find_package)?;
        resolved.push(Package {
            id: id.to_owned(),
            name: string(package, "name")?.to_owned(),
            path,
            member: member_ids.contains(&id),
            dependencies,
            dev_dependencies,
            target_kinds: target_kinds(package)?,
            links: links(package)?,
            features: feature_map(package)?,
            declarations: declared(package)?,
            edges,
            lib_name: lib_name(package)?,
        });
    }
    Ok(Workspace {
        root: PathBuf::from(string(&document, "workspace_root")?),
        target_dir: PathBuf::from(string(&document, "target_directory")?),
        members,
        packages: resolved,
    })
}

/// A resolved node's edges, and the ids of its normal and build
/// dependencies and of those reached only as development dependencies.
type NodeEdges = (Vec<String>, Vec<String>, Vec<Edge>);

fn node_edges<'a>(
    node: &Value,
    find_package: &impl Fn(&str) -> Result<&'a Value, Error>,
) -> Result<NodeEdges, Error> {
    let mut dependencies = Vec::new();
    let mut dev_dependencies = Vec::new();
    let mut edges = Vec::new();
    for dependency in array(node, "deps")? {
        let mut shipped = false;
        let mut kinds = Vec::new();
        for kind in array(dependency, "dep_kinds")? {
            let name = match kind.get("kind") {
                Some(Value::Null) => "normal",
                Some(Value::String(kind)) if kind == "build" => "build",
                Some(Value::String(kind)) if kind == "dev" => "dev",
                _ => return Err(Error::UnexpectedOutput(COMMAND)),
            };
            shipped |= name != "dev";
            let platform = match kind.get("target") {
                Some(Value::Null) => None,
                Some(Value::String(platform)) => Some(platform.clone()),
                _ => return Err(Error::UnexpectedOutput(COMMAND)),
            };
            kinds.push((name.to_owned(), platform));
        }
        let target = string(dependency, "pkg")?;
        find_package(target)?;
        edges.push(Edge {
            name: string(dependency, "name")?.to_owned(),
            package: target.to_owned(),
            kinds,
        });
        if shipped {
            dependencies.push(target.to_owned());
        } else {
            dev_dependencies.push(target.to_owned());
        }
    }
    Ok((dependencies, dev_dependencies, edges))
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

/// The target kinds that make a target a package's library.
const LIBRARY_KINDS: &[&str] = &["lib", "rlib", "dylib", "cdylib", "staticlib", "proc-macro"];

/// The name of a package's library target, if it has one.
fn lib_name(package: &Value) -> Result<Option<String>, Error> {
    for target in array(package, "targets")? {
        if kinds_of(target)?
            .iter()
            .any(|kind| LIBRARY_KINDS.contains(kind))
        {
            return Ok(Some(string(target, "name")?.to_owned()));
        }
    }
    Ok(None)
}

/// Each declaration of `package` with the package its own edge resolved
/// to. A renamed declaration is named in the graph by its rename, and an
/// unrenamed one by its dependency's library, with `-` as `_`; the edge
/// must also carry the declaration's kind and platform, and resolve to a
/// package of the declared name. An edge a renamed declaration takes is
/// not given to an unrenamed one, so each edge binds one declaration of a
/// kind. A declaration Cargo did not resolve, such as an optional one no
/// feature turned on, is bound to nothing.
pub(crate) fn resolved<'a>(
    workspace: &'a Workspace,
    package: &'a Package,
) -> Vec<(&'a Declaration, &'a Package)> {
    let kind_of = |declared: &Declaration| (declared.kind.clone(), declared.target.clone());
    let find = |id: &str| workspace.packages.iter().find(|found| found.id == id);
    let mut taken: Vec<(&str, (String, Option<String>))> = Vec::new();
    let mut bound = Vec::new();
    for renamed in [true, false] {
        for declared in package
            .declarations
            .iter()
            .filter(|declared| declared.renamed == renamed)
        {
            let kind = kind_of(declared);
            for edge in &package.edges {
                let Some(to) = find(&edge.package) else {
                    continue;
                };
                let name = if renamed {
                    Some(declared.name.replace('-', "_"))
                } else {
                    to.lib_name.as_ref().map(|name| name.replace('-', "_"))
                };
                let taken_here = taken
                    .iter()
                    .any(|(name, taken)| *name == edge.name && *taken == kind);
                if to.name == declared.package
                    && name.as_deref() == Some(edge.name.as_str())
                    && edge.kinds.contains(&kind)
                    && !taken_here
                {
                    taken.push((edge.name.as_str(), kind.clone()));
                    bound.push((declared, to));
                }
            }
        }
    }
    bound.sort_by_key(|(declared, _)| {
        package
            .declarations
            .iter()
            .position(|found| std::ptr::eq(found, *declared))
    });
    bound
}

/// The kinds of a package's targets.
fn target_kinds(package: &Value) -> Result<Vec<String>, Error> {
    let mut kinds = Vec::new();
    for target in array(package, "targets")? {
        kinds.extend(kinds_of(target)?.into_iter().map(str::to_owned));
    }
    Ok(kinds)
}

/// The kinds of one target.
fn kinds_of(target: &Value) -> Result<Vec<&str>, Error> {
    array(target, "kind")?
        .iter()
        .map(|kind| kind.as_str().ok_or(Error::UnexpectedOutput(COMMAND)))
        .collect()
}

/// The native library a package links, if any.
fn links(package: &Value) -> Result<Option<String>, Error> {
    match package.get("links") {
        Some(Value::Null) => Ok(None),
        Some(Value::String(links)) => Ok(Some(links.clone())),
        _ => Err(Error::UnexpectedOutput(COMMAND)),
    }
}

/// A package's features, each with what it enables.
fn feature_map(package: &Value) -> Result<Vec<(String, Vec<String>)>, Error> {
    match package.get("features") {
        Some(Value::Object(features)) => features
            .iter()
            .map(|(name, enables)| {
                let enables = enables.as_array().ok_or(Error::UnexpectedOutput(COMMAND))?;
                Ok((name.clone(), strings(enables)?))
            })
            .collect(),
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
