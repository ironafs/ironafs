// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for the generated apt source and the archive signature check.

use super::{check_apt_update, sources};
use crate::cell::Failure;
use crate::cell::definition::parse;
use crate::cell::runtime::tests::{failed, ok};

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
