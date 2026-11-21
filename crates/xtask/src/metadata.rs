// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The workspace's resolved dependency graph, read from `cargo metadata`.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::Error;
use crate::json::{self, Value};

#[cfg(test)]
mod tests;

/// The workspace members and their resolved dependencies.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Workspace {
    pub(crate) root: PathBuf,
    pub(crate) target_dir: PathBuf,
    pub(crate) members: Vec<Member>,
}

/// One workspace member.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Member {
    pub(crate) name: String,
    /// The directory holding the member's manifest.
    pub(crate) dir: PathBuf,
    /// The directory holding the library target's root file, if it has one.
    pub(crate) lib_dir: Option<PathBuf>,
    /// Package names of the normal and build dependencies, on any platform
    /// and with every feature on. Development dependencies are left out:
    /// they never reach a shipped build.
    pub(crate) dependencies: Vec<String>,
}

const COMMAND: &str = "cargo metadata";

/// Runs `cargo metadata` with every feature on and for every platform, and
/// reads the resolved graph. With no manifest it reads this repository,
/// with its lock file held fixed.
///
/// # Errors
///
/// Returns an error if cargo fails or its output is not the expected graph.
pub(crate) fn load(manifest: Option<&Path>) -> Result<Workspace, Error> {
    let mut command = Command::new("cargo");
    command.args(["metadata", "--format-version", "1", "--all-features"]);
    match manifest {
        Some(manifest) => command
            .arg("--offline")
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

    let mut members = Vec::new();
    for id in array(&document, "workspace_members")? {
        let id = id.as_str().ok_or_else(unexpected)?;
        let package = find_package(id)?;
        let manifest = PathBuf::from(string(package, "manifest_path")?);
        let dir = manifest.parent().ok_or_else(unexpected)?.to_path_buf();
        let mut lib_dir = None;
        for target in array(package, "targets")? {
            if array(target, "kind")?
                .iter()
                .any(|kind| kind.as_str() == Some("lib"))
            {
                let source = PathBuf::from(string(target, "src_path")?);
                lib_dir = Some(source.parent().ok_or_else(unexpected)?.to_path_buf());
            }
        }
        let node = nodes
            .iter()
            .find(|node| string(node, "id").is_ok_and(|candidate| candidate == id))
            .ok_or_else(unexpected)?;
        let mut dependencies = Vec::new();
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
            if shipped {
                let target = find_package(string(dependency, "pkg")?)?;
                dependencies.push(string(target, "name")?.to_owned());
            }
        }
        members.push(Member {
            name: string(package, "name")?.to_owned(),
            dir,
            lib_dir,
            dependencies,
        });
    }
    Ok(Workspace {
        root: PathBuf::from(string(&document, "workspace_root")?),
        target_dir: PathBuf::from(string(&document, "target_directory")?),
        members,
    })
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
