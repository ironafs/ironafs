// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for the hash and version checks and the order of the install
//! steps.

use std::fs;

use super::{
    InstallViolation, compare_hashes, compare_installed, install, parse_dpkg_query, parse_sha256sum,
};
use crate::cell::Failure;
use crate::cell::archive::sources;
use crate::cell::definition::{Arch, Definition, Digest, PackagePin, parse};
use crate::cell::runtime::tests::{FakeRun, ok};
use crate::cell::state::{RunDir, State};

const BASE: &str = "base-files 13.8+deb13u1\ndebianutils 5.23.2\nlibc6 2.41-12\n";
const OK: &str = include_str!("../../../tests/fixtures/cell/dpkg-query-ok.txt");
const KDC_FILE: &str = "krb5-kdc_1.21.3-5+deb13u1_amd64.deb";

fn committed() -> Definition {
    parse(include_str!("../../../cell/pins.txt")).unwrap()
}

fn amd64(definition: &Definition) -> Vec<&PackagePin> {
    definition
        .packages
        .iter()
        .filter(|pin| pin.arch == Arch::Amd64)
        .collect()
}

/// `sha256sum` output for the pins, as the builder container prints it.
fn sha256sum_output(pins: &[&PackagePin]) -> String {
    pins.iter().fold(String::new(), |text, pin| {
        text + &format!("{}  /var/cache/cell/{}\n", pin.sha256, pin.local_file())
    })
}

fn listing(text: &str) -> Vec<(String, String)> {
    parse_dpkg_query(text).unwrap()
}

fn flip_first_digit(digest: Digest) -> Digest {
    let mut bytes = digest.0;
    bytes[0] ^= 0x10;
    Digest(bytes)
}

#[test]
fn parse_sha256sum_reads_file_names() {
    let definition = committed();
    let pins = amd64(&definition);

    let measured = parse_sha256sum(&sha256sum_output(&pins)).unwrap();

    assert_eq!(measured.len(), 7);
    assert!(measured.contains(&(KDC_FILE.to_owned(), pins[1].sha256)));
}

#[test]
fn parse_sha256sum_bad_line_reports_unexpected_output() {
    assert_eq!(
        parse_sha256sum("abc  /var/cache/cell/x.deb\n"),
        Err(Failure::UnexpectedOutput {
            step: "sha256sum",
            line: 1
        })
    );
}

#[test]
fn compare_hashes_all_matching_is_clean() {
    let definition = committed();
    let pins = amd64(&definition);
    let measured = parse_sha256sum(&sha256sum_output(&pins)).unwrap();

    assert_eq!(compare_hashes(&pins, &measured), vec![]);
}

#[test]
fn compare_hashes_flipped_digit_reports_hash_mismatch() {
    let definition = committed();
    let pins = amd64(&definition);
    let mut measured = parse_sha256sum(&sha256sum_output(&pins)).unwrap();
    let kdc = measured
        .iter_mut()
        .find(|(file, _)| file == KDC_FILE)
        .unwrap();
    kdc.1 = flip_first_digit(kdc.1);

    let expected =
        Digest::parse_hex("5533f05f8d20cee3e5441b5934f131a3b5b0487c7c1818680f175cbe8c7ea7c1")
            .unwrap();
    assert_eq!(
        compare_hashes(&pins, &measured),
        vec![InstallViolation::HashMismatch {
            file: KDC_FILE.to_owned(),
            expected,
            found: flip_first_digit(expected),
        }]
    );
}

#[test]
fn compare_hashes_missing_file_reports_not_downloaded() {
    let definition = committed();
    let pins = amd64(&definition);
    let mut measured = parse_sha256sum(&sha256sum_output(&pins)).unwrap();
    measured.retain(|(file, _)| file != KDC_FILE);

    assert_eq!(
        compare_hashes(&pins, &measured),
        vec![InstallViolation::NotDownloaded {
            file: KDC_FILE.to_owned()
        }]
    );
}

#[test]
fn compare_hashes_extra_file_reports_unexpected() {
    let definition = committed();
    let pins = amd64(&definition);
    let mut measured = parse_sha256sum(&sha256sum_output(&pins)).unwrap();
    measured.push((
        "libkrb5-3_1.21.3-5+deb13u1_amd64.deb".to_owned(),
        Digest([7; 32]),
    ));

    assert_eq!(
        compare_hashes(&pins, &measured),
        vec![InstallViolation::Unexpected {
            file: "libkrb5-3_1.21.3-5+deb13u1_amd64.deb".to_owned()
        }]
    );
}

#[test]
fn compare_installed_ok_fixture_is_clean() {
    let definition = committed();

    assert_eq!(
        compare_installed(&amd64(&definition), &listing(BASE), &listing(OK)),
        vec![]
    );
}

#[test]
fn compare_installed_extra_package_reports_unpinned() {
    let definition = committed();
    let after = listing(include_str!(
        "../../../tests/fixtures/cell/dpkg-query-extra.txt"
    ));

    assert_eq!(
        compare_installed(&amd64(&definition), &listing(BASE), &after),
        vec![InstallViolation::Unpinned {
            package: "libkrb5-3".to_owned(),
            version: "1.21.3-5+deb13u1".to_owned(),
        }]
    );
}

#[test]
fn compare_installed_changed_base_package_reports_unpinned() {
    let definition = committed();
    let after = listing(&OK.replace("libc6 2.41-12", "libc6 2.41-13"));

    assert_eq!(
        compare_installed(&amd64(&definition), &listing(BASE), &after),
        vec![InstallViolation::Unpinned {
            package: "libc6".to_owned(),
            version: "2.41-13".to_owned(),
        }]
    );
}

#[test]
fn compare_installed_other_version_reports_version_mismatch() {
    let definition = committed();
    let after = listing(&OK.replace("krb5-kdc 1.21.3-5+deb13u1", "krb5-kdc 1.21.3-6"));

    assert_eq!(
        compare_installed(&amd64(&definition), &listing(BASE), &after),
        vec![InstallViolation::VersionMismatch {
            package: "krb5-kdc".to_owned(),
            expected: "1.21.3-5+deb13u1".to_owned(),
            found: "1.21.3-6".to_owned(),
        }]
    );
}

#[test]
fn compare_installed_absent_pin_reports_not_installed() {
    let definition = committed();
    let after = listing(&OK.replace("krb5-kdc 1.21.3-5+deb13u1\n", ""));

    assert_eq!(
        compare_installed(&amd64(&definition), &listing(BASE), &after),
        vec![InstallViolation::NotInstalled {
            package: "krb5-kdc".to_owned()
        }]
    );
}

#[test]
fn parse_dpkg_query_bad_line_reports_unexpected_output() {
    assert_eq!(
        parse_dpkg_query("libc6\n"),
        Err(Failure::UnexpectedOutput {
            step: "dpkg-query",
            line: 1
        })
    );
}

/// A run directory for one install test.
fn run_dir(name: &str) -> (std::path::PathBuf, RunDir) {
    let target = std::env::temp_dir().join(format!(
        "ironafs-xtask-cell-install-{name}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&target);
    let dir = RunDir::create(&target, "r1".to_owned()).unwrap();
    (target, dir)
}

/// A fake engine whose outputs make an amd64 install of the committed
/// pins succeed.
fn clean_engine(definition: &Definition) -> FakeRun {
    let mut run = FakeRun::default();
    run.script("list base packages", ok(BASE.as_bytes()));
    run.script(
        "sha256sum",
        ok(sha256sum_output(&amd64(definition)).as_bytes()),
    );
    run.script("list installed packages", ok(OK.as_bytes()));
    run
}

#[test]
fn install_clean_run_commits_the_image_after_every_check() {
    let definition = committed();
    let (target, dir) = run_dir("clean");
    let mut run = clean_engine(&definition);
    let mut state = State::default();

    let image = install(&mut run, &definition, Arch::Amd64, &dir, &mut state).unwrap();

    assert_eq!(image, "ironafs-cell:r1");
    assert_eq!(
        run.names(),
        vec![
            "pull image",
            "start builder",
            "write sources",
            "apt-get update",
            "list base packages",
            "apt-get download",
            "sha256sum",
            "remove sources",
            "dpkg -i",
            "list installed packages",
            "commit image",
        ]
    );
    assert_eq!(
        fs::read_to_string(dir.root.join("state.txt")).unwrap(),
        "container ironafs-cell-r1-builder\nimage ironafs-cell:r1\n"
    );
    let pull = &run.steps[0].args;
    assert_eq!(
        pull.last().unwrap(),
        "docker.io/library/debian@sha256:bfc36aecdece0387418759dba474aa856c026ca8087da3606b41b90ab0509c99"
    );
    fs::remove_dir_all(&target).unwrap();
}

#[test]
fn install_passes_values_as_arguments_not_script_text() {
    let definition = committed();
    let (target, dir) = run_dir("arguments");
    let mut run = clean_engine(&definition);

    install(
        &mut run,
        &definition,
        Arch::Amd64,
        &dir,
        &mut State::default(),
    )
    .unwrap();

    let download = run
        .steps
        .iter()
        .find(|step| step.name == "apt-get download")
        .unwrap();
    let args: Vec<&str> = download
        .args
        .iter()
        .map(|arg| arg.to_str().unwrap())
        .collect();
    assert!(args.contains(&"krb5-kdc=1.21.3-5+deb13u1"));
    let write = run
        .steps
        .iter()
        .find(|step| step.name == "write sources")
        .unwrap();
    assert_eq!(
        write.stdin.as_deref(),
        Some(sources(&definition).as_bytes())
    );
    fs::remove_dir_all(&target).unwrap();
}

#[test]
fn install_runs_hash_check_before_dpkg() {
    let definition = committed();
    let (target, dir) = run_dir("hash-first");
    let mut run = FakeRun::default();
    run.script("list base packages", ok(BASE.as_bytes()));
    let planted = sha256sum_output(&amd64(&definition)).replace(
        "5533f05f8d20cee3e5441b5934f131a3b5b0487c7c1818680f175cbe8c7ea7c1",
        "4533f05f8d20cee3e5441b5934f131a3b5b0487c7c1818680f175cbe8c7ea7c1",
    );
    run.script("sha256sum", ok(planted.as_bytes()));

    let result = install(
        &mut run,
        &definition,
        Arch::Amd64,
        &dir,
        &mut State::default(),
    );

    let expected =
        Digest::parse_hex("5533f05f8d20cee3e5441b5934f131a3b5b0487c7c1818680f175cbe8c7ea7c1")
            .unwrap();
    assert_eq!(
        result,
        Err(Failure::Install(vec![InstallViolation::HashMismatch {
            file: KDC_FILE.to_owned(),
            expected,
            found: flip_first_digit(expected),
        }]))
    );
    assert_eq!(run.names().last(), Some(&"sha256sum"));
    assert!(!run.names().contains(&"dpkg -i"));
    fs::remove_dir_all(&target).unwrap();
}

#[test]
fn install_unpinned_package_stops_before_commit() {
    let definition = committed();
    let (target, dir) = run_dir("unpinned");
    let mut run = FakeRun::default();
    run.script("list base packages", ok(BASE.as_bytes()));
    run.script(
        "sha256sum",
        ok(sha256sum_output(&amd64(&definition)).as_bytes()),
    );
    run.script(
        "list installed packages",
        ok(include_bytes!(
            "../../../tests/fixtures/cell/dpkg-query-extra.txt"
        )),
    );

    let result = install(
        &mut run,
        &definition,
        Arch::Amd64,
        &dir,
        &mut State::default(),
    );

    assert_eq!(
        result,
        Err(Failure::Install(vec![InstallViolation::Unpinned {
            package: "libkrb5-3".to_owned(),
            version: "1.21.3-5+deb13u1".to_owned(),
        }]))
    );
    assert!(!run.names().contains(&"commit image"));
    fs::remove_dir_all(&target).unwrap();
}

#[test]
fn install_without_platform_pin_reports_missing_arch() {
    let mut definition = committed();
    definition
        .image
        .platforms
        .retain(|(arch, _)| *arch != Arch::Arm64);
    let (target, dir) = run_dir("no-platform");
    let mut run = FakeRun::default();

    let result = install(
        &mut run,
        &definition,
        Arch::Arm64,
        &dir,
        &mut State::default(),
    );

    assert_eq!(
        result,
        Err(Failure::Definition(vec![
            crate::cell::definition::DefinitionError::MissingArch { arch: "arm64" }
        ]))
    );
    assert_eq!(run.steps.len(), 0);
    fs::remove_dir_all(&target).unwrap();
}
