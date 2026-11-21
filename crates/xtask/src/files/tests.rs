// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for the shared filesystem steps.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use super::{remove_dir, staged_path, write_staged};

fn fresh(name: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("ironafs-xtask-files-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn entries(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

/// Describes a staging failure by its operation and kind.
fn describe() -> impl Fn(&'static str, &Path, io::Error) -> String {
    |operation, _, error| format!("{operation}: {}", error.kind())
}

#[test]
fn remove_dir_missing_is_already_removed() {
    let dir = fresh("remove-missing").join("absent");

    assert_eq!(remove_dir(&dir, |error| error.kind()), Ok(()));
}

#[test]
fn remove_dir_removes_contents() {
    let dir = fresh("remove-contents");
    fs::write(dir.join("file"), b"x").unwrap();

    assert_eq!(remove_dir(&dir, |error| error.kind()), Ok(()));
    assert!(!dir.exists());
}

#[test]
fn staged_paths_differ_between_calls() {
    let dir = PathBuf::from("d");

    assert_ne!(staged_path(&dir, "latest"), staged_path(&dir, "latest"));
}

#[test]
fn write_staged_rename_publishes_and_leaves_nothing_staged() {
    let dir = fresh("rename");
    let target = dir.join("latest");

    write_staged(
        &dir,
        "latest",
        |file| file.write_all(b"r1"),
        |staged| fs::rename(staged, &target).map_err(|error| error.to_string()),
        describe(),
    )
    .unwrap();

    assert_eq!(fs::read_to_string(&target).unwrap(), "r1");
    assert_eq!(entries(&dir), ["latest"]);
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn write_staged_failed_publish_removes_the_staged_file() {
    let dir = fresh("publish-fails");

    let result = write_staged(
        &dir,
        "latest",
        |file| file.write_all(b"r1"),
        |_| Err("refused".to_owned()),
        describe(),
    );

    assert_eq!(result, Err("refused".to_owned()));
    assert_eq!(entries(&dir), Vec::<String>::new());
    fs::remove_dir_all(&dir).unwrap();
}
