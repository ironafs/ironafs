// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for writing fixture workspaces.

use std::fs;

use super::{Fixture, remove_dir};

#[test]
fn fixture_writes_manifests_dependencies_and_files() {
    let dir = std::env::temp_dir().join(format!("ironafs-xtask-fixture-{}", std::process::id()));
    remove_dir(&dir).unwrap();
    let mut fixture = Fixture::default();
    fixture.add_crate("ironafs-time", &[]);
    fixture.add_crate("ironafs-xdr", &["ironafs-time", "helper", "bytes"]);
    fixture.dependency_line(
        "ironafs-xdr",
        "other = { path = \"../other\", optional = true }",
    );
    fixture.manifest_tail("ironafs-xdr", "[features]\nextra = [\"dep:other\"]\n");
    fixture.lints("ironafs-time", "\n[lints]\n");
    fixture.edition("ironafs-time", "edition = \"2021\"");
    fixture.file("ironafs-xdr", "src/lib.rs", None);
    fixture.file("ironafs-xdr", "src/main.rs", Some("fn main() {}\n"));
    fixture.add_crate_at("tools/ironafs-cli", "ironafs-cli", &[]);
    fixture.add_outside_crate("helper", &["ironafs-time"]);
    fixture.add_third_party("bytes", &["libc"]);
    fixture.add_third_party("libc", &[]);

    let manifest = fixture.write(&dir).unwrap();

    assert_eq!(manifest, dir.join("workspace/Cargo.toml"));
    let read = |path: &str| fs::read_to_string(dir.join(path)).unwrap();
    assert_eq!(
        read("workspace/Cargo.toml"),
        "[workspace]\nresolver = \"3\"\nmembers = [\"crates/*\"]\n\n\
         [workspace.package]\nedition = \"2024\"\n\n\
         [workspace.lints.rust]\nunsafe_code = \"forbid\"\n"
    );
    assert_eq!(
        read("workspace/crates/ironafs-time/Cargo.toml"),
        "[package]\nname = \"ironafs-time\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\n\
         [dependencies]\n\n[lints]\n"
    );
    assert_eq!(
        read("workspace/crates/ironafs-xdr/Cargo.toml"),
        "[package]\nname = \"ironafs-xdr\"\nversion = \"0.0.0\"\nedition.workspace = true\n\n\
         [dependencies]\nironafs-time = { path = \"../../../workspace/crates/ironafs-time\" }\n\
         helper = { path = \"../../../outside/helper\" }\n\
         bytes = \"1\"\n\
         other = { path = \"../other\", optional = true }\n\
         \n[lints]\nworkspace = true\n\
         [features]\nextra = [\"dep:other\"]\n"
    );
    assert_eq!(
        read("outside/helper/Cargo.toml"),
        "[package]\nname = \"helper\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n\
         [dependencies]\nironafs-time = { path = \"../../workspace/crates/ironafs-time\" }\n"
    );
    assert_eq!(
        read("vendor/bytes/Cargo.toml"),
        "[package]\nname = \"bytes\"\nversion = \"1.0.0\"\nedition = \"2024\"\n\n\
         [dependencies]\nlibc = \"1\"\n"
    );
    assert_eq!(
        read("vendor/bytes/.cargo-checksum.json"),
        "{\"files\":{},\"package\":null}"
    );
    assert!(dir.join("workspace/tools/ironafs-cli/src/lib.rs").is_file());
    assert!(!dir.join("workspace/crates/ironafs-xdr/src/lib.rs").exists());
    assert_eq!(
        read("workspace/crates/ironafs-xdr/src/main.rs"),
        "fn main() {}\n"
    );
    assert_eq!(
        fixture.names(),
        ["ironafs-time", "ironafs-xdr", "ironafs-cli"]
    );
    assert_eq!(
        fixture.config(&dir),
        [
            "source.crates-io.replace-with='fixture'".to_owned(),
            format!(
                "source.fixture.directory='{}'",
                dir.join("vendor").display()
            ),
        ]
    );
    assert_eq!(Fixture::default().config(&dir), [] as [String; 0]);
    remove_dir(&dir).unwrap();
    remove_dir(&dir).unwrap();
}

#[test]
fn remove_dependency_line_drops_only_that_line() {
    let dir = std::env::temp_dir().join(format!(
        "ironafs-xtask-fixture-remove-{}",
        std::process::id()
    ));
    remove_dir(&dir).unwrap();
    let mut fixture = Fixture::default();
    fixture.add_crate("ironafs-time", &[]);
    fixture.add_crate("ironafs-xdr", &["ironafs-time"]);
    fixture.dependency_line("ironafs-xdr", "kept = \"1\"");
    fixture.dependency_line("ironafs-xdr", "dropped = \"1\"");

    fixture.remove_dependency_line("ironafs-xdr", "dropped = \"1\"");

    fixture.write(&dir).unwrap();
    assert_eq!(
        fs::read_to_string(dir.join("workspace/crates/ironafs-xdr/Cargo.toml")).unwrap(),
        "[package]\nname = \"ironafs-xdr\"\nversion = \"0.0.0\"\nedition.workspace = true\n\n\
         [dependencies]\nironafs-time = { path = \"../../../workspace/crates/ironafs-time\" }\n\
         kept = \"1\"\n\
         \n[lints]\nworkspace = true\n"
    );
    remove_dir(&dir).unwrap();
}

#[test]
fn workspace_tail_follows_the_workspace_manifest() {
    let dir =
        std::env::temp_dir().join(format!("ironafs-xtask-fixture-tail-{}", std::process::id()));
    remove_dir(&dir).unwrap();
    let mut fixture = Fixture::default();
    fixture.add_crate("ironafs-time", &[]);
    fixture.workspace_tail("\n[patch.crates-io]\nx = { path = \"crates/x\" }\n");

    let manifest = fixture.write(&dir).unwrap();

    assert!(
        fs::read_to_string(manifest).unwrap().ends_with(
            "unsafe_code = \"forbid\"\n\n[patch.crates-io]\nx = { path = \"crates/x\" }\n"
        )
    );
    remove_dir(&dir).unwrap();
}

#[test]
fn outside_version_replaces_only_the_outside_crate_s_version() {
    let dir = std::env::temp_dir().join(format!(
        "ironafs-xtask-fixture-version-{}",
        std::process::id()
    ));
    remove_dir(&dir).unwrap();
    // The outside crate shares a workspace crate's name: only it changes.
    let mut fixture = Fixture::default();
    fixture.add_crate("helper", &[]);
    fixture.add_outside_crate("helper", &[]);
    fixture.outside_version("helper", "2.0.0");

    fixture.write(&dir).unwrap();

    assert!(
        fs::read_to_string(dir.join("outside/helper/Cargo.toml"))
            .unwrap()
            .starts_with("[package]\nname = \"helper\"\nversion = \"2.0.0\"\n")
    );
    assert!(
        fs::read_to_string(dir.join("workspace/crates/helper/Cargo.toml"))
            .unwrap()
            .starts_with("[package]\nname = \"helper\"\nversion = \"0.0.0\"\n")
    );
    remove_dir(&dir).unwrap();
}

#[test]
fn third_party_tail_changes_only_the_third_party_crate_of_a_shared_name() {
    let dir = std::env::temp_dir().join(format!(
        "ironafs-xtask-fixture-third-party-tail-{}",
        std::process::id()
    ));
    remove_dir(&dir).unwrap();
    let mut fixture = Fixture::default();
    fixture.add_crate("ironafs-disk", &[]);
    fixture.add_third_party("ironafs-disk", &[]);
    fixture.third_party_tail("ironafs-disk", "[features]\nextra = []\n");

    fixture.write(&dir).unwrap();

    let read = |path: &str| fs::read_to_string(dir.join(path)).unwrap();
    assert!(read("vendor/ironafs-disk/Cargo.toml").ends_with("[features]\nextra = []\n"));
    assert!(!read("workspace/crates/ironafs-disk/Cargo.toml").contains("extra"));
    remove_dir(&dir).unwrap();
}
