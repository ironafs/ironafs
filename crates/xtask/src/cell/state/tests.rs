// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for the run directory, the record of created objects, and
//! teardown from that record alone.

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use super::{Object, RunDir, State, down, teardown};
use crate::cell::Failure;
use crate::cell::runtime::tests::{FakeRun, failed};

/// A fresh target directory for one test, private to its process.
fn target(name: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("ironafs-xtask-cell-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    dir
}

/// A run directory holding a key file and these recorded objects.
fn recorded(target: &Path, objects: Vec<Object>) -> (RunDir, State) {
    let dir = RunDir::create(target, "r1".to_owned()).unwrap();
    fs::create_dir(dir.root.join("keys")).unwrap();
    fs::write(dir.root.join("keys").join("rxkad.keytab"), b"key").unwrap();
    let mut state = State::default();
    for object in objects {
        state.record(&dir, object).unwrap();
    }
    (dir, state)
}

fn args(run: &FakeRun) -> Vec<Vec<OsString>> {
    run.steps.iter().map(|step| step.args.clone()).collect()
}

fn os(args: &[&str]) -> Vec<OsString> {
    args.iter().map(OsString::from).collect()
}

fn four_objects() -> Vec<Object> {
    vec![
        Object::Image("i".to_owned()),
        Object::Network("n".to_owned()),
        Object::Container("c1".to_owned()),
        Object::Container("c2".to_owned()),
    ]
}

#[test]
fn teardown_removes_in_reverse_order() {
    let target = target("reverse");
    let (dir, state) = recorded(&target, four_objects());
    let mut run = FakeRun::default();

    teardown(&mut run, &dir, &state).unwrap();

    assert_eq!(
        args(&run),
        vec![
            os(&["rm", "-f", "c2"]),
            os(&["rm", "-f", "c1"]),
            os(&["network", "rm", "n"]),
            os(&["rmi", "i"]),
        ]
    );
    fs::remove_dir_all(&target).unwrap();
}

#[test]
fn teardown_missing_object_is_not_an_error() {
    let target = target("missing");
    let (dir, state) = recorded(&target, four_objects());
    let mut run = FakeRun::default();
    run.script(
        "remove container",
        failed(1, b"Error response from daemon: No such container: c2\n"),
    );
    run.script(
        "remove network",
        failed(
            1,
            b"Error: unable to find network with name or ID n: network not found\n",
        ),
    );
    run.script("remove image", failed(1, b"Error: i: image not known\n"));

    assert_eq!(teardown(&mut run, &dir, &state), Ok(()));
    assert!(!dir.root.exists());
    fs::remove_dir_all(&target).unwrap();
}

#[test]
fn teardown_other_failure_is_reported_and_rest_still_run() {
    let target = target("other-failure");
    let (dir, state) = recorded(&target, four_objects());
    let mut run = FakeRun::default();
    run.script(
        "remove container",
        failed(1, b"Error: cannot remove container c2: device busy\n"),
    );

    let result = teardown(&mut run, &dir, &state);

    assert_eq!(
        result,
        Err(Failure::Teardown(vec![Failure::Step {
            name: "remove container",
            code: Some(1),
            detail: "Error: cannot remove container c2: device busy".to_owned(),
        }]))
    );
    assert_eq!(run.steps.len(), 4);
    // What was not removed stays recorded for the next `down`; the keys go.
    assert_eq!(
        State::load(&dir).unwrap(),
        State {
            objects: vec![Object::Container("c2".to_owned())]
        }
    );
    assert!(!dir.root.join("keys").exists());
    fs::remove_dir_all(&target).unwrap();
}

#[test]
fn teardown_deletes_run_dir() {
    let target = target("deletes");
    let (dir, state) = recorded(&target, four_objects());

    teardown(&mut FakeRun::default(), &dir, &state).unwrap();

    assert!(!dir.root.exists());
    fs::remove_dir_all(&target).unwrap();
}

#[test]
fn down_twice_second_is_noop() {
    let target = target("down-twice");
    let _ = recorded(&target, four_objects());
    let mut run = FakeRun::default();

    assert_eq!(down(&mut run, &target), Ok(Some("r1".to_owned())));
    assert_eq!(run.steps.len(), 4);
    assert_eq!(down(&mut run, &target), Ok(None));
    assert_eq!(run.steps.len(), 4);
    fs::remove_dir_all(&target).unwrap();
}

#[test]
fn down_with_no_run_is_noop() {
    let target = target("down-none");
    let mut run = FakeRun::default();

    assert_eq!(down(&mut run, &target), Ok(None));
    assert_eq!(run.steps.len(), 0);
}

#[test]
fn record_writes_state_before_returning() {
    let target = target("record");
    let dir = RunDir::create(&target, "r1".to_owned()).unwrap();
    let mut state = State::default();

    state
        .record(&dir, Object::Network("ironafs-cell-r1".to_owned()))
        .unwrap();

    assert_eq!(
        fs::read_to_string(dir.root.join("state.txt")).unwrap(),
        "network ironafs-cell-r1\n"
    );
    assert_eq!(State::load(&dir).unwrap(), state);
    fs::remove_dir_all(&target).unwrap();
}

#[test]
fn state_load_refuses_unknown_line() {
    let target = target("unknown-line");
    let dir = RunDir::create(&target, "r1".to_owned()).unwrap();
    fs::write(dir.root.join("state.txt"), "volume v\n").unwrap();

    assert_eq!(
        State::load(&dir),
        Err(Failure::Io {
            operation: "read the cell state file",
            path: dir.root.join("state.txt"),
            error: "line 1 names no container, network or image".to_owned(),
        })
    );
    fs::remove_dir_all(&target).unwrap();
}

#[test]
fn run_dir_create_existing_reports_in_use() {
    let target = target("in-use");
    let first = RunDir::create(&target, "r1".to_owned()).unwrap();

    assert_eq!(
        RunDir::create(&target, "r1".to_owned()),
        Err(Failure::RunDirInUse(first.root))
    );
    fs::remove_dir_all(&target).unwrap();
}

#[test]
fn run_dir_create_new_skips_taken_ids() {
    let target = target("new-id");
    RunDir::create(&target, "2a-0".to_owned()).unwrap();

    let dir = RunDir::create_new(&target, 42).unwrap();

    assert_eq!(dir.id, "2a-1");
    assert_eq!(dir.root, target.join("xtask-cell").join("2a-1"));
    fs::remove_dir_all(&target).unwrap();
}

#[test]
fn run_dir_latest_is_the_last_created() {
    let target = target("latest");
    assert_eq!(RunDir::latest(&target), Ok(None));
    RunDir::create(&target, "r1".to_owned()).unwrap();
    let second = RunDir::create(&target, "r2".to_owned()).unwrap();

    assert_eq!(RunDir::latest(&target), Ok(Some(second)));
    fs::remove_dir_all(&target).unwrap();
}

#[cfg(unix)]
#[test]
fn run_dir_is_private_to_its_owner() {
    use std::os::unix::fs::PermissionsExt;

    let target = target("mode");
    let dir = RunDir::create(&target, "r1".to_owned()).unwrap();

    let mode = fs::metadata(&dir.root).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o700);
    fs::remove_dir_all(&target).unwrap();
}

#[test]
fn run_dir_create_ignores_another_runs_staging_file() {
    let target = target("staging");
    // Another process's marker staged under the old fixed name.
    fs::create_dir_all(target.join("xtask-cell").join("latest.new")).unwrap();

    let dir = RunDir::create(&target, "r1".to_owned()).unwrap();

    assert_eq!(RunDir::latest(&target), Ok(Some(dir)));
    let staged: Vec<_> = fs::read_dir(target.join("xtask-cell"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .filter(|name| name.to_string_lossy().starts_with(".ironafs-"))
        .collect();
    assert_eq!(staged, Vec::<std::ffi::OsString>::new());
    fs::remove_dir_all(&target).unwrap();
}

#[test]
fn run_dir_create_removes_its_directory_when_the_marker_fails() {
    let target = target("marker-fails");
    // A non-empty directory where the marker goes: the rename fails.
    fs::create_dir_all(target.join("xtask-cell").join("latest").join("x")).unwrap();

    let result = RunDir::create(&target, "r1".to_owned());

    assert!(matches!(result, Err(Failure::Io { .. })));
    assert!(!target.join("xtask-cell").join("r1").exists());
    fs::remove_dir_all(&target).unwrap();
}
