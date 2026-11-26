// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for resolving the package closure and writing it back into the
//! pin file.

use std::fs;

use super::{Printed, parse_print_uris, pin, render};
use crate::cell::Failure;
use crate::cell::definition::{Digest, parse};
use crate::cell::runtime::tests::{FakeRun, ok};
use crate::cell::state::{RunDir, State};

const PINS: &str = include_str!("../../../cell/pins.txt");
const SNAPSHOT: &str = "https://snapshot.debian.org/archive/debian/20261120T023438Z";
const AMD64: &str = include_str!("../../../tests/fixtures/cell/print-uris-amd64.txt");
const ARM64: &str = include_str!("../../../tests/fixtures/cell/print-uris-arm64.txt");

fn run_dir(name: &str) -> (std::path::PathBuf, RunDir) {
    let target = std::env::temp_dir().join(format!(
        "ironafs-xtask-cell-pin-{name}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&target);
    let dir = RunDir::create(&target, "r1".to_owned()).unwrap();
    (target, dir)
}

/// The arm64 lines in reverse order, so the output's order is the sort's.
fn arm64_reversed() -> String {
    ARM64
        .lines()
        .rev()
        .fold(String::new(), |text, line| text + line + "\n")
}

/// Pins the committed definition against an engine that resolves the
/// fixtures' closure, and renders the pin file.
fn pin_once(name: &str) -> String {
    let definition = parse(PINS).unwrap();
    let (target, dir) = run_dir(name);
    let mut run = FakeRun::default();
    run.script("apt-get print uris", ok(AMD64.as_bytes()));
    run.script("apt-get print uris", ok(arm64_reversed().as_bytes()));

    let packages = pin(&mut run, &definition, &dir, &mut State::default()).unwrap();

    fs::remove_dir_all(&target).unwrap();
    render(&without_packages(), &packages)
}

/// The committed pin file with its package lines removed.
fn without_packages() -> String {
    PINS.lines()
        .filter(|line| !line.starts_with("package "))
        .fold(String::new(), |text, line| text + line + "\n")
}

#[test]
fn parse_print_uris_decodes_epoch() {
    let line = format!(
        "'{SNAPSHOT}/pool/main/k/krb5-config/krb5-config_2.7_all.deb' krb5-config_1%3a2.7_all.deb 2000 SHA256:{}\n",
        "0".repeat(64)
    );

    assert_eq!(
        parse_print_uris(&line, SNAPSHOT),
        Ok(vec![Printed {
            pool: "pool/main/k/krb5-config/krb5-config_2.7_all.deb".to_owned(),
            name: "krb5-config".to_owned(),
            version: "1:2.7".to_owned(),
            sha256: Digest([0; 32]),
        }])
    );
}

#[test]
fn parse_print_uris_decodes_the_quoted_url() {
    let printed = parse_print_uris(AMD64, SNAPSHOT).unwrap();

    assert_eq!(printed.len(), 7);
    assert_eq!(
        printed[0].pool,
        "pool/main/k/krb5/krb5-admin-server_1.21.3-5+deb13u1_amd64.deb"
    );
    assert_eq!(printed[0].version, "1.21.3-5+deb13u1");
}

#[test]
fn parse_print_uris_url_outside_snapshot_reports_unexpected_output() {
    let moved = AMD64.replacen(SNAPSHOT, "https://deb.debian.org/debian", 1);

    assert_eq!(
        parse_print_uris(&moved, SNAPSHOT),
        Err(Failure::UnexpectedOutput {
            step: "apt-get print uris",
            line: 1
        })
    );
}

#[test]
fn parse_print_uris_other_hash_reports_unexpected_output() {
    let other = AMD64.replacen("SHA256:", "SHA512:", 1);

    assert_eq!(
        parse_print_uris(&other, SNAPSHOT),
        Err(Failure::UnexpectedOutput {
            step: "apt-get print uris",
            line: 1
        })
    );
}

#[test]
fn pin_output_is_sorted_and_stable() {
    let first = pin_once("stable-1");
    let second = pin_once("stable-2");

    assert_eq!(first, second);
    // The fixtures hold exactly the committed closure, so the committed
    // file is reproduced byte for byte.
    assert_eq!(first, PINS);
}

#[test]
fn pin_checks_the_archive_signature_before_resolving() {
    let definition = parse(PINS).unwrap();
    let (target, dir) = run_dir("signature");
    let mut run = FakeRun::default();
    let mut update = ok(b"");
    update.stderr = b"W: GPG error: InRelease: NO_PUBKEY 0000000000000000\n".to_vec();
    run.script("apt-get update", update);

    let result = pin(&mut run, &definition, &dir, &mut State::default());

    assert_eq!(
        result,
        Err(Failure::ArchiveNotVerified(
            "W: GPG error: InRelease: NO_PUBKEY 0000000000000000".to_owned()
        ))
    );
    assert!(!run.names().contains(&"apt-get print uris"));
    fs::remove_dir_all(&target).unwrap();
}

#[test]
fn render_keeps_every_other_line_and_places_packages_once() {
    let definition = parse(PINS).unwrap();
    let text = PINS.replace(
        "cell test.ironafs.invalid",
        "# a note\ncell test.ironafs.invalid",
    );

    let rendered = render(&text, &definition.packages);

    assert_eq!(rendered, text);
}

#[test]
fn render_with_no_package_lines_appends_them() {
    let definition = parse(PINS).unwrap();

    assert_eq!(render(&without_packages(), &definition.packages), PINS);
}
