// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for the pin file grammar and its refusals.

use super::{Arch, DefinitionError, Digest, PackagePin, parse};

const FILESERVER: &str = "openafs-fileserver";

#[track_caller]
fn errors(text: &str) -> Vec<DefinitionError> {
    parse(text).expect_err("the planted pin file parsed")
}

#[test]
fn parse_committed_pins_is_ok() {
    let definition = parse(include_str!("../../../cell/pins.txt")).unwrap();

    assert_eq!(
        definition.image.index.to_string(),
        "9c6e381421d0f48d1b8133e573f33c185ee16cbf505eda9c4a8a4791b25ea8a6"
    );
    assert_eq!(definition.image.repository, "docker.io/library/debian");
    assert_eq!(definition.image.tag, "trixie");
    assert_eq!(
        definition.image.platforms,
        vec![
            (
                Arch::Amd64,
                Digest::parse_hex(
                    "bfc36aecdece0387418759dba474aa856c026ca8087da3606b41b90ab0509c99"
                )
                .unwrap()
            ),
            (
                Arch::Arm64,
                Digest::parse_hex(
                    "e529717ccc8118a0b9d38525f2b4e78cf150dbe470ad60d733c7c0023cabc3b0"
                )
                .unwrap()
            ),
        ]
    );
    assert_eq!(
        definition.snapshot.url(),
        "https://snapshot.debian.org/archive/debian/20261120T023438Z"
    );
    assert_eq!(definition.cell.cell, "test.ironafs.invalid");
    assert_eq!(definition.cell.realm, "TEST.IRONAFS.INVALID");
    let kdc = definition
        .packages
        .iter()
        .find(|pin| pin.arch == Arch::Amd64 && pin.name == "krb5-kdc")
        .unwrap();
    assert_eq!(kdc.version, "1.21.3-5+deb13u1");
    assert_eq!(
        kdc.sha256.to_string(),
        "5533f05f8d20cee3e5441b5934f131a3b5b0487c7c1818680f175cbe8c7ea7c1"
    );
    assert_eq!(kdc.local_file(), "krb5-kdc_1.21.3-5+deb13u1_amd64.deb");
}

#[test]
fn parse_version_with_star_reports_floating() {
    assert_eq!(
        errors(include_str!(
            "../../../tests/fixtures/cell/pins-floating.txt"
        )),
        vec![DefinitionError::FloatingVersion {
            line: 14,
            name: FILESERVER.into()
        }]
    );
}

#[test]
fn parse_short_digest_reports_bad_digest() {
    assert_eq!(
        errors(include_str!(
            "../../../tests/fixtures/cell/pins-short-digest.txt"
        )),
        vec![DefinitionError::BadDigest { line: 10 }]
    );
}

#[test]
fn parse_url_with_userinfo_reports_credentials() {
    assert_eq!(
        errors(include_str!(
            "../../../tests/fixtures/cell/pins-userinfo.txt"
        )),
        vec![DefinitionError::CredentialsInUrl { line: 7 }]
    );
}

#[test]
fn parse_url_with_query_reports_credentials() {
    assert_eq!(
        errors(include_str!("../../../tests/fixtures/cell/pins-query.txt")),
        vec![DefinitionError::CredentialsInUrl { line: 7 }]
    );
}

#[test]
fn parse_trusted_yes_reports_insecure_option() {
    assert_eq!(
        errors(include_str!(
            "../../../tests/fixtures/cell/pins-trusted.txt"
        )),
        vec![DefinitionError::InsecureOption { line: 7 }]
    );
}

#[test]
fn parse_bracketed_option_reports_insecure_option() {
    let text = include_str!("../../../cell/pins.txt").replace(
        "snapshot https://",
        "snapshot [signed-by=/dev/null] https://",
    );

    assert_eq!(
        errors(&text),
        vec![DefinitionError::InsecureOption {
            line: line_of(&text, "snapshot ")
        }]
    );
}

#[test]
fn parse_missing_arm64_pin_reports_missing_arch() {
    assert_eq!(
        errors(include_str!(
            "../../../tests/fixtures/cell/pins-no-arm64.txt"
        )),
        vec![DefinitionError::MissingArch { arch: "arm64" }]
    );
}

#[test]
fn parse_fileserver_at_other_version_reports_required_pin_wrong() {
    assert_eq!(
        errors(include_str!(
            "../../../tests/fixtures/cell/pins-other-version.txt"
        )),
        vec![DefinitionError::RequiredPinWrong {
            arch: "amd64",
            name: FILESERVER,
            expected: "1.8.13.2-1",
            found: Some("1.8.14-1".into()),
        }]
    );
}

#[test]
fn parse_absent_required_package_reports_required_pin_wrong() {
    let text = without_lines(&committed(), "package arm64 krb5-user ");

    assert_eq!(
        errors(&text),
        vec![DefinitionError::RequiredPinWrong {
            arch: "arm64",
            name: "krb5-user",
            expected: "1.21.3-5+deb13u1",
            found: None,
        }]
    );
}

#[test]
fn parse_real_looking_cell_reports_not_synthetic() {
    assert_eq!(
        errors(include_str!(
            "../../../tests/fixtures/cell/pins-real-cell.txt"
        )),
        vec![DefinitionError::NotSynthetic { line: 8 }]
    );
}

#[test]
fn parse_duplicate_package_reports_duplicate() {
    assert_eq!(
        errors(include_str!(
            "../../../tests/fixtures/cell/pins-duplicate.txt"
        )),
        vec![DefinitionError::Duplicate {
            line: 11,
            what: "package amd64 krb5-kdc".into()
        }]
    );
}

#[test]
fn parse_errors_are_all_reported_not_first_only() {
    assert_eq!(
        errors(include_str!(
            "../../../tests/fixtures/cell/pins-two-errors.txt"
        )),
        vec![
            DefinitionError::BadDigest { line: 10 },
            DefinitionError::FloatingVersion {
                line: 14,
                name: FILESERVER.into()
            },
        ]
    );
}

#[test]
fn parse_empty_text_reports_every_missing_entry() {
    assert_eq!(
        errors(""),
        vec![
            DefinitionError::Missing("image"),
            DefinitionError::Missing("snapshot"),
            DefinitionError::Missing("cell"),
            DefinitionError::MissingArch { arch: "amd64" },
            DefinitionError::MissingArch { arch: "arm64" },
        ]
    );
}

#[test]
fn parse_unknown_key_reports_unknown_key() {
    let text = format!("{}mirror https://example.invalid\n", committed());

    assert_eq!(
        errors(&text),
        vec![DefinitionError::UnknownKey {
            line: line_of(&text, "mirror ")
        }]
    );
}

#[test]
fn parse_filename_for_another_package_reports_malformed() {
    let text = committed().replace(
        "pool/main/k/krb5/krb5-kdc_1.21.3-5+deb13u1_amd64.deb",
        "pool/main/k/krb5/krb5-user_1.21.3-5+deb13u1_amd64.deb",
    );

    assert_eq!(
        errors(&text),
        vec![DefinitionError::Malformed {
            line: line_of(&text, "package amd64 krb5-kdc ")
        }]
    );
}

#[test]
fn parse_package_all_expands_to_both_arches() {
    let text = without_lines(&committed(), "package amd64 krb5-kdc ");
    let text = without_lines(&text, "package arm64 krb5-kdc ")
        + &format!(
            "package all krb5-kdc 1.21.3-5+deb13u1 {} pool/main/k/krb5/krb5-kdc_1.21.3-5+deb13u1_all.deb\n",
            "0".repeat(64)
        );

    let definition = parse(&text).unwrap();

    let arches: Vec<Arch> = definition
        .packages
        .iter()
        .filter(|pin| pin.name == "krb5-kdc")
        .map(|pin| pin.arch)
        .collect();
    assert_eq!(arches, vec![Arch::Amd64, Arch::Arm64]);
}

#[test]
fn digest_parse_hex_refuses_uppercase_and_wrong_length() {
    let lower = "ab".repeat(32);
    assert!(Digest::parse_hex(&lower).is_some());
    assert_eq!(Digest::parse_hex(&lower.to_uppercase()), None);
    assert_eq!(Digest::parse_hex(&"ab".repeat(31)), None);
    assert_eq!(Digest::parse_hex(&format!("{lower}0")), None);
    assert_eq!(Digest::parse_hex(&"zz".repeat(32)), None);
}

#[test]
fn parse_epoch_version_matches_pool_file_without_epoch() {
    let text = committed().replace(
        "package amd64 krb5-user 1.21.3-5+deb13u1",
        "package amd64 krb5-user 1:1.21.3-5+deb13u1",
    );

    // The pool file name carries no epoch, so the line parses; the
    // required version check then reports the other version.
    assert_eq!(
        errors(&text),
        vec![DefinitionError::RequiredPinWrong {
            arch: "amd64",
            name: "krb5-user",
            expected: "1.21.3-5+deb13u1",
            found: Some("1:1.21.3-5+deb13u1".into()),
        }]
    );
}

#[test]
fn local_file_quotes_the_epoch_and_keeps_the_deb_arch() {
    let pin = PackagePin {
        arch: Arch::Arm64,
        name: "krb5-config".into(),
        version: "1:2.7".into(),
        sha256: Digest([0; 32]),
        filename: "pool/main/k/krb5-config/krb5-config_2.7_all.deb".into(),
    };

    assert_eq!(pin.local_file(), "krb5-config_1%3a2.7_all.deb");
}

fn committed() -> String {
    include_str!("../../../cell/pins.txt").to_owned()
}

/// `text` without the lines starting with `prefix`.
fn without_lines(text: &str, prefix: &str) -> String {
    text.lines()
        .filter(|line| !line.starts_with(prefix))
        .fold(String::new(), |kept, line| kept + line + "\n")
}

/// The 1-based number of the first line starting with `prefix`.
#[track_caller]
fn line_of(text: &str, prefix: &str) -> usize {
    text.lines()
        .position(|line| line.starts_with(prefix))
        .unwrap()
        + 1
}

#[test]
fn arch_host_matches_the_build_target() {
    let expected = if cfg!(target_arch = "x86_64") {
        Some(Arch::Amd64)
    } else if cfg!(target_arch = "aarch64") {
        Some(Arch::Arm64)
    } else {
        None
    };

    assert_eq!(Arch::host(), expected);
}

#[test]
fn load_returns_the_committed_text_and_its_definition() {
    let text = include_str!("../../../cell/pins.txt");

    assert_eq!(super::load(), Ok((text.to_owned(), parse(text).unwrap())));
}
