// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Throwaway workspaces for the rule checks' self-tests, written under the
//! target directory and removed afterwards.

use std::fs;
use std::path::{Path, PathBuf};

use crate::Error;
use crate::metadata::{self, Workspace};

#[cfg(test)]
mod tests;

/// One crate of a fixture: a workspace crate, a local crate outside the
/// workspace, or a third-party crate served from a local registry copy.
#[derive(Clone, Debug)]
struct FixtureCrate {
    name: String,
    place: Place,
    dependencies: Vec<Dependency>,
    /// The `[package]` table's edition line.
    edition: String,
    /// Lines added to the `[package]` table.
    package_lines: String,
    manifest_tail: String,
    /// The `[lints]` table of a workspace crate; by default it inherits the
    /// workspace's.
    lints: String,
    files: Vec<(String, String)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Place {
    /// In the workspace, at this directory relative to its root.
    Workspace(String),
    /// A local crate outside the workspace root.
    Outside,
    /// A third-party crate.
    ThirdParty,
}

#[derive(Clone, Debug)]
enum Dependency {
    /// A normal dependency on another crate of the fixture, by name.
    On(String),
    /// A line written as given.
    Line(String),
}

/// A workspace of crates under `crates/`, each with an empty `src/lib.rs`
/// unless a case changes it, and the crates around it.
#[derive(Clone, Debug, Default)]
pub(crate) struct Fixture {
    crates: Vec<FixtureCrate>,
    /// Files outside every crate, relative to the workspace root.
    root_files: Vec<(String, String)>,
}

/// Where the workspace, the local crates outside it and the third-party
/// crates are written, below a fixture's directory.
const WORKSPACE: &str = "workspace";
const OUTSIDE: &str = "outside";
const VENDOR: &str = "vendor";

/// The workspace manifest: every crate under `crates/`, with the edition
/// and the lints the crates inherit.
const WORKSPACE_MANIFEST: &str = "[workspace]\nresolver = \"3\"\nmembers = [\"crates/*\"]\n\n\
                                  [workspace.package]\nedition = \"2024\"\n\n\
                                  [workspace.lints.rust]\nunsafe_code = \"forbid\"\n";

/// A workspace crate's `[lints]` table unless a case changes it.
const INHERITED_LINTS: &str = "\n[lints]\nworkspace = true\n";

impl Fixture {
    /// Adds a workspace crate under `crates/` with dependencies on the named
    /// crates.
    pub(crate) fn add_crate(&mut self, name: &str, dependencies: &[&str]) {
        self.add(
            name,
            Place::Workspace(format!("crates/{name}")),
            dependencies,
        );
    }

    /// Adds a workspace crate at `dir`, relative to the workspace root. A
    /// crate outside `crates/` joins the workspace when a member depends on
    /// it by path.
    pub(crate) fn add_crate_at(&mut self, dir: &str, name: &str, dependencies: &[&str]) {
        self.add(name, Place::Workspace(dir.to_owned()), dependencies);
    }

    /// Adds a local crate outside the workspace root.
    pub(crate) fn add_outside_crate(&mut self, name: &str, dependencies: &[&str]) {
        self.add(name, Place::Outside, dependencies);
    }

    /// Adds a third-party crate, version 1.0.0, served from a local copy
    /// of the registry. Its dependencies must be third-party crates.
    pub(crate) fn add_third_party(&mut self, name: &str, dependencies: &[&str]) {
        self.add(name, Place::ThirdParty, dependencies);
    }

    fn add(&mut self, name: &str, place: Place, dependencies: &[&str]) {
        // A workspace crate inherits the workspace's edition; a crate
        // outside it cannot.
        let edition = if matches!(place, Place::Workspace(_)) {
            "edition.workspace = true"
        } else {
            "edition = \"2024\""
        };
        self.crates.push(FixtureCrate {
            name: name.to_owned(),
            place,
            edition: edition.to_owned(),
            dependencies: Vec::new(),
            package_lines: String::new(),
            manifest_tail: String::new(),
            lints: INHERITED_LINTS.to_owned(),
            files: vec![("src/lib.rs".to_owned(), String::new())],
        });
        for dependency in dependencies {
            self.depend(name, dependency);
        }
    }

    /// Adds a normal dependency on another crate of the fixture.
    pub(crate) fn depend(&mut self, from: &str, to: &str) {
        if let Some(found) = self.find(from) {
            found.dependencies.push(Dependency::On(to.to_owned()));
        }
    }

    /// Adds a raw line to a crate's `[dependencies]` table.
    pub(crate) fn dependency_line(&mut self, name: &str, line: &str) {
        if let Some(found) = self.find(name) {
            found.dependencies.push(Dependency::Line(line.to_owned()));
        }
    }

    /// Removes a raw line that `dependency_line` added to a crate.
    pub(crate) fn remove_dependency_line(&mut self, name: &str, line: &str) {
        if let Some(found) = self.find(name) {
            found.dependencies.retain(
                |dependency| !matches!(dependency, Dependency::Line(existing) if existing == line),
            );
        }
    }

    /// Adds a line to a crate's `[package]` table.
    pub(crate) fn package_line(&mut self, name: &str, line: &str) {
        if let Some(found) = self.find(name) {
            found.package_lines.push_str(line);
            found.package_lines.push('\n');
        }
    }

    /// Replaces the edition line of a crate's `[package]` table.
    pub(crate) fn edition(&mut self, name: &str, line: &str) {
        if let Some(found) = self.find(name) {
            line.clone_into(&mut found.edition);
        }
    }

    /// Appends tables after a crate's `[dependencies]` table.
    pub(crate) fn manifest_tail(&mut self, name: &str, text: &str) {
        if let Some(found) = self.find(name) {
            found.manifest_tail.push_str(text);
        }
    }

    /// Replaces a workspace crate's `[lints]` table with `text`.
    pub(crate) fn lints(&mut self, name: &str, text: &str) {
        if let Some(found) = self.find(name) {
            text.clone_into(&mut found.lints);
        }
    }

    /// Sets a file of a crate, relative to its directory; `None` removes it.
    pub(crate) fn file(&mut self, name: &str, path: &str, contents: Option<&str>) {
        if let Some(found) = self.find(name) {
            found.files.retain(|(existing, _)| existing != path);
            if let Some(contents) = contents {
                found.files.push((path.to_owned(), contents.to_owned()));
            }
        }
    }

    /// Sets a file relative to the workspace root, outside every crate.
    pub(crate) fn root_file(&mut self, path: &str, contents: &str) {
        self.root_files.retain(|(existing, _)| existing != path);
        self.root_files.push((path.to_owned(), contents.to_owned()));
    }

    /// The names of the workspace crates, in the order they were added.
    pub(crate) fn names(&self) -> Vec<String> {
        self.crates
            .iter()
            .filter(|found| matches!(found.place, Place::Workspace(_)))
            .map(|found| found.name.clone())
            .collect()
    }

    fn find(&mut self, name: &str) -> Option<&mut FixtureCrate> {
        self.crates.iter_mut().find(|found| found.name == name)
    }

    /// A crate's directory relative to the fixture's directory.
    fn dir_of(found: &FixtureCrate) -> String {
        match &found.place {
            Place::Workspace(dir) => format!("{WORKSPACE}/{dir}"),
            Place::Outside => format!("{OUTSIDE}/{}", found.name),
            Place::ThirdParty => format!("{VENDOR}/{}", found.name),
        }
    }

    /// The manifest line for a dependency of `from` on the crate `to`.
    fn dependency_text(&self, from: &FixtureCrate, to: &str) -> String {
        let target = self.crates.iter().find(|found| found.name == to);
        match target {
            Some(found) if found.place == Place::ThirdParty => format!("{to} = \"1\""),
            _ => {
                let up = "../".repeat(Self::dir_of(from).split('/').count());
                let dir = target.map_or_else(|| format!("{WORKSPACE}/crates/{to}"), Self::dir_of);
                format!("{to} = {{ path = \"{up}{dir}\" }}")
            }
        }
    }

    /// Writes the fixture into an empty `dir` and returns the workspace's
    /// manifest.
    ///
    /// # Errors
    ///
    /// Returns an error if a file cannot be written.
    pub(crate) fn write(&self, dir: &Path) -> Result<PathBuf, Error> {
        let manifest = dir.join(WORKSPACE).join("Cargo.toml");
        write_file(&manifest, WORKSPACE_MANIFEST)?;
        for (path, contents) in &self.root_files {
            write_file(&dir.join(WORKSPACE).join(path), contents)?;
        }
        for found in &self.crates {
            let crate_dir = dir.join(Self::dir_of(found));
            let version = if found.place == Place::ThirdParty {
                "1.0.0"
            } else {
                "0.0.0"
            };
            let mut text = format!(
                "[package]\nname = \"{}\"\nversion = \"{version}\"\n{}\n{}\n[dependencies]\n",
                found.name, found.edition, found.package_lines
            );
            for dependency in &found.dependencies {
                match dependency {
                    Dependency::On(to) => text.push_str(&self.dependency_text(found, to)),
                    Dependency::Line(line) => text.push_str(line),
                }
                text.push('\n');
            }
            if matches!(found.place, Place::Workspace(_)) {
                text.push_str(&found.lints);
            }
            text.push_str(&found.manifest_tail);
            write_file(&crate_dir.join("Cargo.toml"), &text)?;
            for (path, contents) in &found.files {
                write_file(&crate_dir.join(path), contents)?;
            }
            if found.place == Place::ThirdParty {
                write_file(
                    &crate_dir.join(".cargo-checksum.json"),
                    "{\"files\":{},\"package\":null}",
                )?;
            }
        }
        Ok(manifest)
    }

    /// The `--config` values that serve the third-party crates from the
    /// fixture's local registry copy, if it has any.
    pub(crate) fn config(&self, dir: &Path) -> Vec<String> {
        if self
            .crates
            .iter()
            .all(|found| found.place != Place::ThirdParty)
        {
            return Vec::new();
        }
        vec![
            "source.crates-io.replace-with='fixture'".to_owned(),
            format!("source.fixture.directory='{}'", dir.join(VENDOR).display()),
        ]
    }
}

impl Fixture {
    /// Writes the fixture into `dir`, reads it back through `cargo metadata`
    /// and confirms that every workspace crate written was read.
    ///
    /// # Errors
    ///
    /// Returns an error if the fixture cannot be written or read, or a
    /// self-test failure naming `check` and `case` if a crate is missing.
    pub(crate) fn load(
        &self,
        dir: &Path,
        check: &'static str,
        case: &'static str,
    ) -> Result<Workspace, Error> {
        let workspace = metadata::load(Some(&self.write(dir)?), &self.config(dir))?;
        let mut read: Vec<String> = workspace
            .members
            .iter()
            .map(|member| member.name.clone())
            .collect();
        let mut written = self.names();
        read.sort();
        written.sort();
        if read != written {
            return Err(Error::SelfTest {
                check,
                case,
                found: vec![format!(
                    "cargo metadata read crates {read:?}, the fixture has {written:?}"
                )],
            });
        }
        Ok(workspace)
    }
}

/// Writes `contents` to `path`, creating its parent directories.
///
/// # Errors
///
/// Returns an error naming the path if a directory or the file cannot be
/// written.
pub(crate) fn write_file(path: &Path, contents: &str) -> Result<(), Error> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(Error::path_io("create fixture directory", parent))?;
    }
    fs::write(path, contents).map_err(Error::path_io("write fixture file", path))
}

/// A fresh, empty directory for one check's self-test fixtures, private to
/// this process.
///
/// # Errors
///
/// Returns an error if an earlier directory cannot be removed.
pub(crate) fn fresh_dir(target_dir: &Path, check: &str) -> Result<PathBuf, Error> {
    let dir = target_dir
        .join("xtask-self-test")
        .join(format!("{check}-{}", std::process::id()));
    remove_dir(&dir)?;
    Ok(dir)
}

/// Removes a fixture directory; a missing one is already removed.
///
/// # Errors
///
/// Returns an error if the directory exists and cannot be removed.
pub(crate) fn remove_dir(dir: &Path) -> Result<(), Error> {
    crate::files::remove_dir(dir, Error::path_io("remove fixture directory", dir))
}
