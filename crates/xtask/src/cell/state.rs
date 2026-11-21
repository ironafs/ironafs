// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! A cell run's private directory, the record of every engine object the
//! run creates, and teardown from that record alone.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::Failure;
use super::runtime::{Run, Step, require_success};

#[cfg(test)]
mod tests;

/// One run's directory, `<target>/xtask-cell/<id>/`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RunDir {
    pub(crate) root: PathBuf,
    pub(crate) id: String,
}

/// An engine object a run creates.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Object {
    Container(String),
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "TODO(#47): bring-up makes the cell's network")
    )]
    Network(String),
    Image(String),
}

/// The objects a run has recorded, in the order recorded.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct State {
    objects: Vec<Object>,
}

/// The directory under the target directory that holds every run.
const RUNS: &str = "xtask-cell";

/// The file under [`RUNS`] naming the last run created.
const LATEST: &str = "latest";

/// The file in a run directory recording its objects.
const STATE_FILE: &str = "state.txt";

/// The directory in a run directory holding its keys.
pub(crate) const KEYS: &str = "keys";

/// How many ids [`RunDir::create_new`] tries for one process.
const ID_ATTEMPTS: u32 = 1000;

impl RunDir {
    /// Creates the run directory for `id`, private to its owner on Unix,
    /// and marks it the latest run.
    ///
    /// # Errors
    ///
    /// Returns [`Failure::RunDirInUse`] if it already exists, and
    /// [`Failure::Io`] if it or the latest-run marker cannot be written.
    pub(crate) fn create(target_dir: &Path, id: String) -> Result<Self, Failure> {
        let runs = target_dir.join(RUNS);
        fs::create_dir_all(&runs).map_err(Failure::io("create the cell runs directory", &runs))?;
        let root = runs.join(&id);
        match private_dir_builder().create(&root) {
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                return Err(Failure::RunDirInUse(root));
            }
            result => result.map_err(Failure::io("create the run directory", &root))?,
        }
        let latest = runs.join(LATEST);
        write_replacing(&latest, &id, "mark the latest cell run")?;
        Ok(Self { root, id })
    }

    /// Creates a run directory with a new id, `<pid>-<n>` (the pid in
    /// hexadecimal), taking the first `n` whose directory does not exist.
    /// An id is not secret, and it is unique on this target directory only
    /// while earlier runs' directories remain: a run's objects outlive its
    /// directory only if teardown failed, and then the directory stays.
    ///
    /// # Errors
    ///
    /// Returns [`Failure::RunDirInUse`] if every id tried is taken, and
    /// [`Failure::Io`] as [`RunDir::create`] does.
    pub(crate) fn create_new(target_dir: &Path, pid: u32) -> Result<Self, Failure> {
        let mut last = None;
        for attempt in 0..ID_ATTEMPTS {
            match Self::create(target_dir, format!("{pid:x}-{attempt}")) {
                Err(Failure::RunDirInUse(root)) => last = Some(root),
                result => return result,
            }
        }
        Err(Failure::RunDirInUse(
            last.unwrap_or_else(|| target_dir.join(RUNS)),
        ))
    }

    /// The last run created, if its directory still exists.
    ///
    /// # Errors
    ///
    /// Returns [`Failure::Io`] if the latest-run marker cannot be read or
    /// names no valid id.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "TODO(#47): `cell down` tears down the latest run")
    )]
    pub(crate) fn latest(target_dir: &Path) -> Result<Option<Self>, Failure> {
        let runs = target_dir.join(RUNS);
        let marker = runs.join(LATEST);
        let id = match fs::read_to_string(&marker) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            result => result.map_err(Failure::io("read the latest cell run", &marker))?,
        };
        let valid = !id.is_empty()
            && id
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
        if !valid {
            return Err(Failure::Io {
                operation: "read the latest cell run",
                path: marker,
                error: "not a run id".to_owned(),
            });
        }
        let root = runs.join(&id);
        Ok(root.is_dir().then_some(Self { root, id }))
    }

    fn state_file(&self) -> PathBuf {
        self.root.join(STATE_FILE)
    }
}

impl Object {
    const fn kind(&self) -> &'static str {
        match self {
            Self::Container(_) => "container",
            Self::Network(_) => "network",
            Self::Image(_) => "image",
        }
    }

    fn name(&self) -> &str {
        match self {
            Self::Container(name) | Self::Network(name) | Self::Image(name) => name,
        }
    }

    /// The engine step that removes the object.
    fn removal(&self) -> Step {
        match self {
            Self::Container(name) => Step::new("remove container", ["rm", "-f", name]),
            Self::Network(name) => Step::new("remove network", ["network", "rm", name]),
            Self::Image(name) => Step::new("remove image", ["rmi", name]),
        }
    }

    /// Whether `stderr` from a failed removal says the object is already
    /// gone. Docker and Podman word it differently.
    fn already_gone(&self, stderr: &[u8]) -> bool {
        let text = String::from_utf8_lossy(stderr).to_lowercase();
        let markers: &[&str] = match self {
            Self::Container(_) => &["no such container"],
            Self::Network(_) => &["no such network", "network not found"],
            Self::Image(_) => &["no such image", "image not known"],
        };
        markers.iter().any(|marker| text.contains(marker))
    }
}

impl State {
    /// Appends `object` and rewrites the state file. Call it before the
    /// object is created, so an interruption leaves nothing unrecorded.
    ///
    /// # Errors
    ///
    /// Returns [`Failure::Io`] if the state file cannot be written; the
    /// object must then not be created.
    pub(crate) fn record(&mut self, dir: &RunDir, object: Object) -> Result<(), Failure> {
        self.objects.push(object);
        self.save(dir)
    }

    fn save(&self, dir: &RunDir) -> Result<(), Failure> {
        let text = self.objects.iter().fold(String::new(), |text, object| {
            text + object.kind() + " " + object.name() + "\n"
        });
        write_replacing(&dir.state_file(), &text, "write the cell state file")
    }

    /// Reads a run's state file; a missing one records nothing.
    ///
    /// # Errors
    ///
    /// Returns [`Failure::Io`] if the file cannot be read or holds a line
    /// that names no object.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "TODO(#47): `cell down` tears down the latest run")
    )]
    pub(crate) fn load(dir: &RunDir) -> Result<Self, Failure> {
        let path = dir.state_file();
        let text = match fs::read_to_string(&path) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Self::default()),
            result => result.map_err(Failure::io("read the cell state file", &path))?,
        };
        let objects = text
            .lines()
            .enumerate()
            .map(|(index, line)| match line.split_once(' ') {
                Some(("container", name)) => Ok(Object::Container(name.to_owned())),
                Some(("network", name)) => Ok(Object::Network(name.to_owned())),
                Some(("image", name)) => Ok(Object::Image(name.to_owned())),
                _ => Err(Failure::Io {
                    operation: "read the cell state file",
                    path: path.clone(),
                    error: format!("line {} names no container, network or image", index + 1),
                }),
            })
            .collect::<Result<_, _>>()?;
        Ok(Self { objects })
    }
}

/// Removes a run's objects: containers, then networks, then images, each
/// kind in the reverse of the order recorded. An object already gone counts
/// as removed. Every removal is tried, whatever failed before it.
///
/// If everything was removed, the run directory goes too, keys included.
/// Otherwise the keys are removed, the state file is rewritten to record
/// only what remains, so `cargo xtask cell down` can try again, and every
/// failure is reported.
///
/// # Errors
///
/// Returns [`Failure::Teardown`] listing every failure.
pub(crate) fn teardown(run: &mut impl Run, dir: &RunDir, state: &State) -> Result<(), Failure> {
    let mut failures = Vec::new();
    let mut remaining = Vec::new();
    for kind in ["container", "network", "image"] {
        for object in state
            .objects
            .iter()
            .rev()
            .filter(|object| object.kind() == kind)
        {
            let step = object.removal();
            let removed = run.run(&step).and_then(|ran| {
                if !ran.success && object.already_gone(&ran.stderr) {
                    Ok(())
                } else {
                    require_success(&step, &ran)
                }
            });
            if let Err(failure) = removed {
                failures.push(failure);
                remaining.push(object.clone());
            }
        }
    }
    if failures.is_empty() {
        remove_dir(&dir.root, &mut failures);
    } else {
        remaining.reverse();
        if let Err(failure) = (State { objects: remaining }).save(dir) {
            failures.push(failure);
        }
        remove_dir(&dir.root.join(KEYS), &mut failures);
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(Failure::Teardown(failures))
    }
}

/// Tears down the latest run, if one remains.
///
/// # Errors
///
/// Returns a failure if the run cannot be found, read or torn down.
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "TODO(#47): `cell down` tears down the latest run")
)]
pub(crate) fn down(run: &mut impl Run, target_dir: &Path) -> Result<Option<String>, Failure> {
    let Some(dir) = RunDir::latest(target_dir)? else {
        return Ok(None);
    };
    let state = State::load(&dir)?;
    teardown(run, &dir, &state)?;
    Ok(Some(dir.id))
}

/// A directory builder that makes directories private to their owner on
/// Unix. On Windows a new directory inherits its parent's access list.
fn private_dir_builder() -> fs::DirBuilder {
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;

        let mut builder = fs::DirBuilder::new();
        builder.mode(0o700);
        builder
    }
    #[cfg(not(unix))]
    {
        fs::DirBuilder::new()
    }
}

/// Removes a directory and everything in it; a missing one is already
/// removed. This is `fixture::remove_dir` with the cell's error type.
fn remove_dir(dir: &Path, failures: &mut Vec<Failure>) {
    match fs::remove_dir_all(dir) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => {
            failures.push(Failure::io("remove the run directory", dir)(error));
        }
        _ => {}
    }
}

/// Writes `contents` to `path` through a temporary file and a rename, so a
/// reader sees the old contents or the new, never a partial file.
fn write_replacing(path: &Path, contents: &str, operation: &'static str) -> Result<(), Failure> {
    let staged = path.with_extension("new");
    fs::write(&staged, contents).map_err(Failure::io(operation, &staged))?;
    fs::rename(&staged, path).map_err(Failure::io(operation, path))
}
