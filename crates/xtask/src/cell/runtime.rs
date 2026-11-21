// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The container engine the cell runs on, driven as a child process, and
//! kept from every registry credential on the host.

use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::thread;

use super::Failure;
use crate::Command;

/// The environment variable that picks the engine: `docker` or `podman`.
pub(crate) const ENGINE_VARIABLE: &str = "IRONAFS_CELL_ENGINE";

/// How many lines of a failed step's error output are reported.
const DETAIL_LINES: usize = 20;

#[cfg(test)]
pub(super) mod tests;

/// A container engine with a Docker-compatible command line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Engine {
    Docker,
    Podman,
}

/// One engine invocation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Step {
    /// What the step does, as reported when it fails.
    pub(crate) name: &'static str,
    /// The arguments after the engine's program name.
    pub(crate) args: Vec<OsString>,
    pub(crate) stdin: Option<Vec<u8>>,
    /// A step that handles key material: its output is never reported.
    pub(crate) secret: bool,
}

/// What a step returned.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Ran {
    pub(crate) success: bool,
    pub(crate) code: Option<i32>,
    pub(crate) stdout: Vec<u8>,
    pub(crate) stderr: Vec<u8>,
}

/// Runs steps; tests replace it with a fake.
pub(crate) trait Run {
    /// Runs one step to completion.
    ///
    /// # Errors
    ///
    /// Returns a failure if the step cannot be started; a step that runs
    /// and fails is a [`Ran`] without `success`.
    fn run(&mut self, step: &Step) -> Result<Ran, Failure>;
}

impl Engine {
    /// Picks the engine: the one `env_override` names, otherwise Docker,
    /// then Podman, whichever `on_path` finds first.
    ///
    /// # Errors
    ///
    /// Returns [`Failure::EngineNotFound`] if the override names neither
    /// engine, or the engine wanted is not on `PATH`.
    pub(crate) fn choose(
        env_override: Option<&OsStr>,
        on_path: impl Fn(&str) -> bool,
    ) -> Result<Self, Failure> {
        let Some(value) = env_override else {
            return [Self::Docker, Self::Podman]
                .into_iter()
                .find(|engine| on_path(engine.program()))
                .ok_or(Failure::EngineNotFound("docker or podman on PATH"));
        };
        let (engine, wanted) = match value.to_str() {
            Some("docker") => (Self::Docker, "docker on PATH (IRONAFS_CELL_ENGINE)"),
            Some("podman") => (Self::Podman, "podman on PATH (IRONAFS_CELL_ENGINE)"),
            _ => {
                return Err(Failure::EngineNotFound(
                    "IRONAFS_CELL_ENGINE set to docker or podman",
                ));
            }
        };
        if on_path(engine.program()) {
            Ok(engine)
        } else {
            Err(Failure::EngineNotFound(wanted))
        }
    }

    /// The engine's program name.
    pub(crate) const fn program(self) -> &'static str {
        match self {
            Self::Docker => "docker",
            Self::Podman => "podman",
        }
    }
}

impl Step {
    /// A public step with no input.
    pub(crate) fn new<const N: usize>(name: &'static str, args: [&str; N]) -> Self {
        Self {
            name,
            args: args.into_iter().map(OsString::from).collect(),
            stdin: None,
            secret: false,
        }
    }
}

/// The real engine, run with no registry credentials: every pull is
/// anonymous.
///
/// Docker reads credentials and credential helpers from the `config.json`
/// in `DOCKER_CONFIG`, and Podman from `REGISTRY_AUTH_FILE` (falling back
/// to Docker's file). Each step gets an empty directory and an empty
/// credential file instead, and `DOCKER_AUTH_CONFIG` is removed. `DOCKER_HOST`
/// is kept, so a rootless Podman or a remote engine socket still works.
#[derive(Debug)]
pub(crate) struct EngineRun {
    engine: Engine,
    config: PathBuf,
    auth: PathBuf,
}

impl EngineRun {
    /// Writes the empty configuration directory and credential file under
    /// `dir/engine/`.
    ///
    /// # Errors
    ///
    /// Returns [`Failure::Io`] if either cannot be written.
    pub(crate) fn new(engine: Engine, dir: &Path) -> Result<Self, Failure> {
        let config = dir.join("engine").join("docker-config");
        let auth = dir.join("engine").join("registry-auth.json");
        fs::create_dir_all(&config)
            .map_err(Failure::io("create the engine config directory", &config))?;
        fs::write(&auth, "{}").map_err(Failure::io(
            "write the empty registry credential file",
            &auth,
        ))?;
        Ok(Self {
            engine,
            config,
            auth,
        })
    }

    /// The engine command for `step`, with the credential isolation set.
    pub(crate) fn command(&self, step: &Step) -> Command {
        let mut command = Command::new(self.engine.program());
        command
            .args(&step.args)
            .env("DOCKER_CONFIG", &self.config)
            .env("REGISTRY_AUTH_FILE", &self.auth)
            .env_remove("DOCKER_AUTH_CONFIG");
        command
    }
}

impl Run for EngineRun {
    fn run(&mut self, step: &Step) -> Result<Ran, Failure> {
        let program = self.engine.program();
        let mut child = self
            .command(step)
            .stdin(if step.stdin.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| {
                if error.kind() == io::ErrorKind::NotFound {
                    Failure::EngineNotFound("docker or podman on PATH")
                } else {
                    Failure::io("start the container engine", Path::new(program))(error)
                }
            })?;
        let pipe = child.stdin.take();
        // The input is written from its own thread, so a step that fills
        // its output pipe before reading all its input cannot deadlock.
        let (written, output) = thread::scope(|scope| {
            let writer = scope.spawn(|| match (pipe, step.stdin.as_deref()) {
                (Some(mut pipe), Some(input)) => pipe.write_all(input),
                _ => Ok(()),
            });
            let output = child.wait_with_output();
            (writer.join(), output)
        });
        let output = output.map_err(Failure::io(
            "wait for the container engine",
            Path::new(program),
        ))?;
        match written {
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                return Err(Failure::io("write a step's input", Path::new(program))(
                    error,
                ));
            }
            Err(_) => {
                return Err(Failure::Io {
                    operation: "write a step's input",
                    path: PathBuf::from(program),
                    error: "the writer thread panicked".to_owned(),
                });
            }
        }
        Ok(Ran {
            success: output.status.success(),
            code: output.status.code(),
            stdout: output.stdout,
            stderr: output.stderr,
        })
    }
}

/// Turns a failed step into [`Failure::Step`].
///
/// # Errors
///
/// Returns [`Failure::Step`] with the last lines of the step's error
/// output, or none of it for a secret step.
pub(crate) fn require_success(step: &Step, ran: &Ran) -> Result<(), Failure> {
    if ran.success {
        return Ok(());
    }
    let detail = if step.secret {
        "(output withheld: secret step)".to_owned()
    } else {
        let stderr = String::from_utf8_lossy(&ran.stderr);
        let lines: Vec<&str> = stderr.lines().collect();
        let tail = lines
            .get(lines.len().saturating_sub(DETAIL_LINES)..)
            .unwrap_or_default();
        tail.join("\n")
    };
    Err(Failure::Step {
        name: step.name,
        code: ran.code,
        detail,
    })
}

/// Runs `step` and requires it to succeed; returns its standard output.
///
/// # Errors
///
/// Returns the failure to start the step, or [`Failure::Step`].
pub(crate) fn run_step(run: &mut impl Run, step: &Step) -> Result<String, Failure> {
    let ran = run.run(step)?;
    require_success(step, &ran)?;
    Ok(String::from_utf8_lossy(&ran.stdout).into_owned())
}
