// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for the generated apt source and the archive signature check.

use std::fs;

use super::{check_apt_update, prepare, sources};
use crate::cell::Failure;
use crate::cell::definition::{Arch, parse};
use crate::cell::runtime::tests::{FakeRun, failed, ok};
use crate::cell::state::{RunDir, State};

#[test]
fn sources_has_signed_by_and_no_trusted() {
    let definition = parse(include_str!("../../../cell/pins.txt")).unwrap();

    assert_eq!(
        sources(&definition),
        "Types: deb\n\
         URIs: https://snapshot.debian.org/archive/debian/20261120T023438Z\n\
         Suites: trixie\n\
         Components: main\n\
         Signed-By: /usr/share/keyrings/debian-archive-keyring.gpg\n"
    );
}

#[test]
fn apt_update_output_with_gpg_error_is_refused() {
    let mut ran = ok(b"");
    ran.stderr = include_bytes!("../../../tests/fixtures/cell/apt-update-gpg-error.txt").to_vec();

    assert_eq!(
        check_apt_update(&ran),
        Err(Failure::ArchiveNotVerified(
            "  The following signatures couldn't be verified because the public key is not \
             available: NO_PUBKEY 0000000000000000"
                .to_owned()
        ))
    );
}

#[test]
fn apt_update_warning_on_stdout_is_refused() {
    let ran = ok(b"Hit:1 https://snapshot.invalid trixie InRelease\nW: some warning\n");

    assert_eq!(
        check_apt_update(&ran),
        Err(Failure::ArchiveNotVerified("W: some warning".to_owned()))
    );
}

#[test]
fn apt_update_nonzero_status_is_refused() {
    assert_eq!(
        check_apt_update(&failed(100, b"E: failed to fetch\n")),
        Err(Failure::Step {
            name: "apt-get update",
            code: Some(100),
            detail: "E: failed to fetch".to_owned(),
        })
    );
}

#[test]
fn apt_update_clean_output_is_accepted() {
    let ran =
        ok(b"Get:1 https://snapshot.invalid trixie InRelease [140 kB]\nReading package lists...\n");

    assert_eq!(check_apt_update(&ran), Ok(()));
}

#[test]
fn prepare_records_the_base_image_before_pulling() {
    let definition = parse(include_str!("../../../cell/pins.txt")).unwrap();
    let target = std::env::temp_dir().join(format!(
        "ironafs-xtask-cell-archive-base-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&target);
    let dir = RunDir::create(&target, "r1".to_owned()).unwrap();
    let mut run = FakeRun::default();
    run.script("pull image", failed(1, b"Error: interrupted\n"));

    let result = prepare(
        &mut run,
        &definition,
        Arch::Amd64,
        &dir,
        &mut State::default(),
        "ironafs-cell-r1-builder",
    );

    assert!(matches!(
        result,
        Err(Failure::Step {
            name: "pull image",
            ..
        })
    ));
    // A pull that fails or is interrupted part-way is still torn down.
    assert_eq!(
        fs::read_to_string(dir.root.join("state.txt")).unwrap(),
        "image docker.io/library/debian@sha256:\
         bfc36aecdece0387418759dba474aa856c026ca8087da3606b41b90ab0509c99\n"
    );
    fs::remove_dir_all(&target).unwrap();
}
