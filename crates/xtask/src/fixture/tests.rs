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
    fixture.add_crate("ironafs-xdr", &["ironafs-time"]);
    fixture.dependency_line(
        "ironafs-xdr",
        "other = { path = \"../other\", optional = true }",
    );
    fixture.manifest_tail("ironafs-xdr", "[features]\nextra = [\"dep:other\"]\n");
    fixture.file("ironafs-xdr", "src/lib.rs", None);
    fixture.file("ironafs-xdr", "src/main.rs", Some("fn main() {}\n"));

    let manifest = fixture.write(&dir).unwrap();

    assert_eq!(manifest, dir.join("Cargo.toml"));
    assert_eq!(
        fs::read_to_string(dir.join("crates/ironafs-xdr/Cargo.toml")).unwrap(),
        "[package]\nname = \"ironafs-xdr\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n\
         [dependencies]\nironafs-time = { path = \"../ironafs-time\" }\n\
         other = { path = \"../other\", optional = true }\n\
         [features]\nextra = [\"dep:other\"]\n"
    );
    assert!(!dir.join("crates/ironafs-xdr/src/lib.rs").exists());
    assert_eq!(
        fs::read_to_string(dir.join("crates/ironafs-xdr/src/main.rs")).unwrap(),
        "fn main() {}\n"
    );
    assert_eq!(fixture.names(), ["ironafs-time", "ironafs-xdr"]);
    remove_dir(&dir).unwrap();
    remove_dir(&dir).unwrap();
}
