// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Throwaway workspaces for the rule checks' self-tests, written under the
//! target directory and removed afterwards.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::Error;
use crate::metadata::{self, Workspace};

#[cfg(test)]
mod tests;

/// One crate of a fixture workspace.
#[derive(Clone, Debug)]
struct FixtureCrate {
    name: String,
    dependencies: Vec<String>,
    manifest_tail: String,
    files: Vec<(String, String)>,
}

/// A workspace of crates under `crates/`, each with an empty `src/lib.rs`
/// unless a case changes it.
#[derive(Clone, Debug, Default)]
pub(crate) struct Fixture {
    crates: Vec<FixtureCrate>,
}

impl Fixture {
    /// Adds a crate with path dependencies on the named crates.
    pub(crate) fn add_crate(&mut self, name: &str, dependencies: &[&str]) {
        self.crates.push(FixtureCrate {
            name: name.to_owned(),
            dependencies: Vec::new(),
            manifest_tail: String::new(),
            files: vec![("src/lib.rs".to_owned(), String::new())],
        });
        for dependency in dependencies {
            self.depend(name, dependency);
        }
    }

    /// Adds a normal path dependency.
    pub(crate) fn depend(&mut self, from: &str, to: &str) {
        self.dependency_line(from, &format!("{to} = {{ path = \"../{to}\" }}"));
    }

    /// Adds a raw line to a crate's `[dependencies]` table.
    pub(crate) fn dependency_line(&mut self, name: &str, line: &str) {
        if let Some(found) = self.find(name) {
            found.dependencies.push(line.to_owned());
        }
    }

    /// Appends tables after a crate's `[dependencies]` table.
    pub(crate) fn manifest_tail(&mut self, name: &str, text: &str) {
        if let Some(found) = self.find(name) {
            found.manifest_tail.push_str(text);
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

    /// The crate names, in the order they were added.
    pub(crate) fn names(&self) -> Vec<String> {
        self.crates.iter().map(|found| found.name.clone()).collect()
    }

    fn find(&mut self, name: &str) -> Option<&mut FixtureCrate> {
        self.crates.iter_mut().find(|found| found.name == name)
    }

    /// Writes the workspace into an empty `dir` and returns its manifest.
    ///
    /// # Errors
    ///
    /// Returns an error if a file cannot be written.
    pub(crate) fn write(&self, dir: &Path) -> Result<PathBuf, Error> {
        let manifest = dir.join("Cargo.toml");
        write_file(
            &manifest,
            "[workspace]\nresolver = \"3\"\nmembers = [\"crates/*\"]\n",
        )?;
        for found in &self.crates {
            let crate_dir = dir.join("crates").join(&found.name);
            let mut text = format!(
                "[package]\nname = \"{}\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n[dependencies]\n",
                found.name
            );
            for line in &found.dependencies {
                text.push_str(line);
                text.push('\n');
            }
            text.push_str(&found.manifest_tail);
            write_file(&crate_dir.join("Cargo.toml"), &text)?;
            for (path, contents) in &found.files {
                write_file(&crate_dir.join(path), contents)?;
            }
        }
        Ok(manifest)
    }
}

impl Fixture {
    /// Writes the fixture into `dir`, reads it back through `cargo metadata`
    /// and confirms that every crate written was read.
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
        let workspace = metadata::load(Some(&self.write(dir)?))?;
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

fn write_file(path: &Path, contents: &str) -> Result<(), Error> {
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
    match fs::remove_dir_all(dir) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => {
            Err(Error::path_io("remove fixture directory", dir)(error))
        }
        _ => Ok(()),
    }
}
