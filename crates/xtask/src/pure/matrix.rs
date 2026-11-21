// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The exact matrix of the pure-crate build, by identity. The expected set
//! is every (crate, target, profile) triple of the pure crates present in
//! the workspace, the toolchain's targets and both profiles. The compiled
//! set is read from the compiler's own outputs: cargo's artifact messages
//! say where each library's metadata was written, which names its target
//! and profile, and the compiler's dependency-info file beside it lists the
//! files it read, which must include the library root. The check passes
//! only if the two sets are equal, so a skipped crate, target or profile,
//! an extra build, or a build that read nothing fails it.

use std::collections::BTreeSet;
use std::fmt;
use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::deps::{self, Kind};
use crate::json::Value;
use crate::metadata::Workspace;

#[cfg(test)]
mod tests;

/// One library build: the crate, the target triple and the profile.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Triple {
    pub(crate) name: String,
    pub(crate) target: String,
    pub(crate) profile: String,
}

impl fmt::Display for Triple {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "({}, {}, {})",
            self.name, self.target, self.profile
        )
    }
}

/// How the compiled set differs from the expected one.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Gap {
    Missing(Triple),
    Extra(Triple),
    /// The dependency-info file is missing or does not list the library
    /// root among the files the compiler read.
    NothingRead {
        triple: Triple,
        dep_info: PathBuf,
    },
}

impl fmt::Display for Gap {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing(triple) => write!(
                formatter,
                "{triple} was not compiled: the check covers every pure crate, target and \
                 profile, or it proves nothing about the one it skipped"
            ),
            Self::Extra(triple) => write!(
                formatter,
                "{triple} was compiled but is not in the expected matrix of pure crates, \
                 targets and profiles"
            ),
            Self::NothingRead { triple, dep_info } => write!(
                formatter,
                "{triple}: the compiler's dependency-info file {} is missing or does not list \
                 the library root, so nothing shows the crate was compiled",
                dep_info.display()
            ),
        }
    }
}

/// A library the compiler reported building.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Built {
    pub(crate) triple: Triple,
    /// The compiler's dependency-info file for this build.
    pub(crate) dep_info: PathBuf,
    /// The library root cargo gave the compiler.
    pub(crate) lib_root: PathBuf,
}

/// The pure crates of ADR 0004, C4, point 1: the one checked-in list, the
/// crate table of `check-deps`.
pub(crate) fn pure_list() -> Vec<&'static str> {
    deps::CRATES
        .iter()
        .filter(|(_, kind)| *kind == Kind::Pure)
        .map(|(name, _)| *name)
        .collect()
}

/// Every triple of `names`, `targets` and `profiles`.
pub(crate) fn expected(names: &[&str], targets: &[String], profiles: &[&str]) -> BTreeSet<Triple> {
    let mut set = BTreeSet::new();
    for name in names {
        for target in targets {
            for profile in profiles {
                set.insert(Triple {
                    name: (*name).to_owned(),
                    target: target.clone(),
                    profile: (*profile).to_owned(),
                });
            }
        }
    }
    set
}

/// The library build a cargo JSON message reports: a `compiler-artifact`
/// for the library target of a workspace member. Its metadata file lies
/// below `build_dir` at `<target>/<profile directory>/`; one written
/// anywhere else gets `?` for what it lacks, so it can match no expected
/// triple.
pub(crate) fn built(message: &Value, workspace: &Workspace, build_dir: &Path) -> Option<Built> {
    if message.get("reason").and_then(Value::as_str) != Some("compiler-artifact") {
        return None;
    }
    let id = message.get("package_id").and_then(Value::as_str)?;
    let member = workspace.members.iter().find(|member| member.id == id)?;
    let target = message.get("target")?;
    let is_library = target
        .get("kind")
        .and_then(Value::as_array)?
        .iter()
        .any(|kind| matches!(kind.as_str(), Some("lib" | "rlib")));
    if !is_library {
        return None;
    }
    let lib_root = PathBuf::from(target.get("src_path").and_then(Value::as_str)?);
    let metadata = message
        .get("filenames")
        .and_then(Value::as_array)?
        .iter()
        .filter_map(Value::as_str)
        .map(Path::new)
        .find(|file| {
            file.extension()
                .is_some_and(|extension| extension == "rmeta")
        })?;
    let stem = metadata.file_stem()?.to_str()?;
    let dep_info = metadata.with_file_name(format!("{}.d", stem.strip_prefix("lib")?));
    let mut parts = metadata.strip_prefix(build_dir).ok().map(Path::components);
    let mut next = || match parts.as_mut().and_then(Iterator::next) {
        Some(Component::Normal(part)) => part.to_str().map(str::to_owned),
        _ => None,
    };
    let triple_name = next().unwrap_or_else(|| "?".to_owned());
    let profile = match next().as_deref() {
        Some("debug") => "dev",
        Some("release") => "release",
        _ => "?",
    };
    Some(Built {
        triple: Triple {
            name: member.name.clone(),
            target: triple_name,
            profile: profile.to_owned(),
        },
        dep_info,
        lib_root,
    })
}

/// The files a dependency-info file says the compiler read: the
/// prerequisites of its rules, in order and once each, relative paths
/// taken from `root`, where cargo runs the compiler.
pub(crate) fn files_read(text: &str, root: &Path) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = Vec::new();
    for line in text.lines() {
        if line.starts_with('#') {
            continue;
        }
        let Some((_, prerequisites)) = line.split_once(": ") else {
            continue;
        };
        let mut word = String::new();
        let mut escaped = false;
        let mut words = Vec::new();
        for character in prerequisites.chars() {
            match character {
                '\\' if !escaped => escaped = true,
                ' ' if !escaped => words.push(std::mem::take(&mut word)),
                _ => {
                    if escaped && character != ' ' {
                        word.push('\\');
                    }
                    escaped = false;
                    word.push(character);
                }
            }
        }
        words.push(word);
        for word in words.into_iter().filter(|word| !word.is_empty()) {
            let path = root.join(word);
            if !found.contains(&path) {
                found.push(path);
            }
        }
    }
    found
}

/// The gap a build shows when its dependency-info file, `text` (`None`
/// when the file is missing), does not list its library root among the
/// files the compiler read; paths are compared in their canonical forms
/// where both exist.
pub(crate) fn read_gap(found: &Built, text: Option<&str>, root: &Path) -> Option<Gap> {
    let read = text.map_or_else(Vec::new, |text| files_read(text, root));
    let same = |file: &PathBuf| match (fs::canonicalize(file), fs::canonicalize(&found.lib_root)) {
        (Ok(one), Ok(other)) => one == other,
        _ => *file == found.lib_root,
    };
    (!read.iter().any(same)).then(|| Gap::NothingRead {
        triple: found.triple.clone(),
        dep_info: found.dep_info.clone(),
    })
}

/// The triples expected and not compiled, then those compiled and not
/// expected.
pub(crate) fn compare(expected: &BTreeSet<Triple>, compiled: &BTreeSet<Triple>) -> Vec<Gap> {
    expected
        .difference(compiled)
        .cloned()
        .map(Gap::Missing)
        .chain(compiled.difference(expected).cloned().map(Gap::Extra))
        .collect()
}

/// The compiled set, one line per crate and profile with its targets.
pub(crate) fn matrix_lines(set: &BTreeSet<Triple>) -> Vec<String> {
    let groups: BTreeSet<(&str, &str)> = set
        .iter()
        .map(|triple| (triple.name.as_str(), triple.profile.as_str()))
        .collect();
    groups
        .into_iter()
        .map(|(name, profile)| {
            let targets: Vec<&str> = set
                .iter()
                .filter(|triple| triple.name == name && triple.profile == profile)
                .map(|triple| triple.target.as_str())
                .collect();
            format!("{name}, {profile}: {}", targets.join(", "))
        })
        .collect()
}
