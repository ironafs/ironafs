// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for choosing the container engine, isolating it from the host's
//! registry credentials, and reporting a failed step; and the `FakeRun`
//! double the other cell modules' tests drive.

use std::collections::{BTreeMap, VecDeque};
use std::ffi::{OsStr, OsString};
use std::fs;

use super::{Engine, EngineRun, Ran, Run, Step, require_success, with_input};
use crate::cell::Failure;

/// A [`Run`] that records every step and replays scripted results by step
/// name, in order. An unscripted step succeeds with no output.
#[derive(Debug, Default)]
pub(in crate::cell) struct FakeRun {
    pub(in crate::cell) steps: Vec<Step>,
    scripted: BTreeMap<&'static str, VecDeque<Ran>>,
}

impl FakeRun {
    /// Queues `ran` as the result of the next step called `name`.
    pub(in crate::cell) fn script(&mut self, name: &'static str, ran: Ran) {
        self.scripted.entry(name).or_default().push_back(ran);
    }

    /// The names of the steps run, in order.
    pub(in crate::cell) fn names(&self) -> Vec<&'static str> {
        self.steps.iter().map(|step| step.name).collect()
    }
}

impl Run for FakeRun {
    fn run(&mut self, step: &Step) -> Result<Ran, Failure> {
        self.steps.push(step.clone());
        Ok(self
            .scripted
            .get_mut(step.name)
            .and_then(VecDeque::pop_front)
            .unwrap_or_else(|| ok(b"")))
    }
}

/// A successful result with `stdout`.
pub(in crate::cell) fn ok(stdout: &[u8]) -> Ran {
    Ran {
        success: true,
        code: Some(0),
        stdout: stdout.to_vec(),
        stderr: Vec::new(),
    }
}

/// A failed result with exit code `code` and `stderr`.
pub(in crate::cell) fn failed(code: i32, stderr: &[u8]) -> Ran {
    Ran {
        success: false,
        code: Some(code),
        stdout: Vec::new(),
        stderr: stderr.to_vec(),
    }
}

#[test]
fn engine_absent_reports_not_found() {
    assert_eq!(
        Engine::choose(None, |_| false),
        Err(Failure::EngineNotFound("docker or podman on PATH"))
    );
}

#[test]
fn engine_default_prefers_docker_then_podman() {
    assert_eq!(Engine::choose(None, |_| true), Ok(Engine::Docker));
    assert_eq!(
        Engine::choose(None, |name| name == "podman"),
        Ok(Engine::Podman)
    );
}

#[test]
fn engine_override_podman_is_chosen_even_if_docker_on_path() {
    assert_eq!(
        Engine::choose(Some(OsStr::new("podman")), |_| true),
        Ok(Engine::Podman)
    );
}

#[test]
fn engine_override_absent_from_path_reports_not_found() {
    assert_eq!(
        Engine::choose(Some(OsStr::new("podman")), |name| name == "docker"),
        Err(Failure::EngineNotFound(
            "podman on PATH (IRONAFS_CELL_ENGINE)"
        ))
    );
}

#[test]
fn engine_override_unknown_value_reports_not_found() {
    assert_eq!(
        Engine::choose(Some(OsStr::new("nerdctl")), |_| true),
        Err(Failure::EngineNotFound(
            "IRONAFS_CELL_ENGINE set to docker or podman"
        ))
    );
}

#[test]
fn engine_command_sets_empty_docker_config() {
    let dir =
        std::env::temp_dir().join(format!("ironafs-xtask-cell-engine-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);

    let engine = EngineRun::new(Engine::Podman, &dir).unwrap();
    let step = Step::new("version", ["version"]);
    let command = engine.command(&step);

    let envs: BTreeMap<&OsStr, Option<&OsStr>> = command.get_envs().collect();
    let config = dir.join("engine").join("docker-config");
    let auth = dir.join("engine").join("registry-auth.json");
    assert_eq!(command.get_program(), "podman");
    assert_eq!(
        command.get_args().collect::<Vec<_>>(),
        [OsStr::new("version")]
    );
    assert_eq!(
        envs.get(OsStr::new("DOCKER_CONFIG")),
        Some(&Some(config.as_os_str()))
    );
    assert_eq!(
        envs.get(OsStr::new("REGISTRY_AUTH_FILE")),
        Some(&Some(auth.as_os_str()))
    );
    assert_eq!(envs.get(OsStr::new("DOCKER_AUTH_CONFIG")), Some(&None));
    assert_eq!(fs::read_dir(&config).unwrap().count(), 0);
    assert_eq!(fs::read(&auth).unwrap(), b"{}");
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn require_success_passes_a_successful_step() {
    let step = Step::new("pull", ["pull"]);

    assert_eq!(require_success(&step, &ok(b"")), Ok(()));
}

#[test]
fn require_success_secret_step_withholds_output() {
    let step = Step {
        secret: true,
        ..Step::new("ktadd", ["exec"])
    };

    assert_eq!(
        require_success(&step, &failed(1, b"KEY=abcd\n")),
        Err(Failure::Step {
            name: "ktadd",
            code: Some(1),
            detail: "(output withheld: secret step)".to_owned(),
        })
    );
}

#[test]
fn require_success_public_step_keeps_last_20_lines() {
    let stderr = (1..=25).fold(String::new(), |text, line| text + &format!("line {line}\n"));
    let step = Step::new("apt-get update", ["exec"]);

    let expected: Vec<String> = (6..=25).map(|line| format!("line {line}")).collect();
    assert_eq!(
        require_success(&step, &failed(100, stderr.as_bytes())),
        Err(Failure::Step {
            name: "apt-get update",
            code: Some(100),
            detail: expected.join("\n"),
        })
    );
}

#[test]
fn step_new_takes_its_arguments_in_order() {
    let step = Step::new("run", ["run", "--rm"]);

    assert_eq!(step.args, [OsString::from("run"), OsString::from("--rm")]);
    assert_eq!(step.stdin, None);
    assert!(!step.secret);
}

#[test]
fn failed_input_write_keeps_the_childs_status_and_output() {
    let ran = Ran {
        stdout: b"partial\n".to_vec(),
        ..failed(1, b"Error: container c is not running\n")
    };

    assert_eq!(
        with_input(ran, Err("Broken pipe".to_owned())),
        Ran {
            success: false,
            code: Some(1),
            stdout: b"partial\n".to_vec(),
            stderr: b"Error: container c is not running\n\
                      write the step's input: Broken pipe\n"
                .to_vec(),
        }
    );
}

#[test]
fn failed_input_write_fails_a_step_that_exited_zero() {
    assert_eq!(
        with_input(ok(b"done\n"), Err("Broken pipe".to_owned())),
        Ran {
            success: false,
            code: Some(0),
            stdout: b"done\n".to_vec(),
            stderr: b"write the step's input: Broken pipe\n".to_vec(),
        }
    );
}

#[test]
fn written_input_leaves_the_result_unchanged() {
    assert_eq!(with_input(ok(b"done\n"), Ok(())), ok(b"done\n"));
}
