// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Release mode and structural input tests on actual argv shapes.

use std::path::Path;

use super::{HARNESS, Invocation, Lto, classify};

const TARGET: &str = "x86_64-unknown-linux-gnu";

fn invocation_as(
    name: &str,
    target: &str,
    verified: bool,
    extra: &[&str],
) -> Result<Invocation, &'static str> {
    let (kind, emit) = match name {
        "afs" => ("bin", "dep-info,link"),
        HARNESS => ("lib", "dep-info,metadata,link,asm"),
        _ => ("lib", "dep-info,metadata,link"),
    };
    let base = [
        "--crate-name",
        name,
        "--edition=2024",
        "fixture with spaces/src/lib.rs",
        "--crate-type",
        kind,
        "--emit",
        emit,
        "--target",
        target,
        "--out-dir",
        "/fresh build/release/out",
        "-Copt-level=3",
        "-Cembed-bitcode=no",
        "-C",
        "overflow-checks=on",
        "-Cmetadata=123",
        "-Cextra-filename=-abc",
        "--check-cfg",
        "cfg(feature, values())",
        "--cap-lints",
        "allow",
    ];
    let argv: Vec<_> = base
        .into_iter()
        .chain(extra.iter().copied())
        .map(str::to_owned)
        .collect();
    classify(&argv, TARGET, &[Path::new("/fresh build")], verified)
}

fn library(extra: &[&str]) -> Invocation {
    invocation_as("ironafs_os", TARGET, true, extra).unwrap()
}

#[test]
fn incremental_paths_compare_equal_but_enablement_must_match() {
    let one = library(&["-C", "incremental=/fresh build/cache one"]);
    let two = library(&["-Cincremental=/fresh build/cache two"]);
    assert_eq!(one.units(), 256);
    assert_eq!(two.compare(&one), Ok(()));
    let off = library(&["-Ccodegen-units=256"]);
    assert_eq!(
        one.compare(&off).unwrap_err(),
        "ironafs_os is not built under the shipping profile (different incremental)"
    );
}

#[test]
fn accumulated_target_features_compare_in_argument_order() {
    let first = library(&["-Ctarget-feature=+sse2", "-C", "target-feature=-sse2"]);
    let combined = library(&["-Ctarget-feature=+sse2,-sse2"]);
    assert_eq!(first.compare(&combined), Ok(()));
    let reversed = library(&["-Ctarget-feature=-sse2", "-Ctarget-feature=+sse2"]);
    assert_eq!(
        reversed.compare(&first).unwrap_err(),
        "ironafs_os is not built under the shipping profile (different target-feature)"
    );
}

#[test]
fn singleton_options_use_the_last_value_and_normalize_spellings() {
    let one = library(&[
        "-Copt-level=1",
        "-Copt_level=3",
        "-Coverflow_checks=yes",
        "-Cdebuginfo=limited",
    ]);
    let two = library(&[
        "-Coverflow-checks=off",
        "-C",
        "overflow-checks=y",
        "-Cdebuginfo=1",
    ]);
    assert_eq!(one.compare(&two), Ok(()));
}

#[test]
fn cfg_values_and_target_identity_cannot_disappear() {
    let one = library(&["--cfg", "feature=\"one\""]);
    let two = library(&["--cfg=feature=\"two\""]);
    assert_eq!(
        two.compare(&one).unwrap_err(),
        "ironafs_os is not built under the shipping profile (different explicit cfg)"
    );
    assert_eq!(
        invocation_as("ironafs_os", "aarch64-unknown-linux-gnu", true, &[]).unwrap_err(),
        "rustc invocation has the wrong target"
    );
    assert_eq!(
        invocation_as("afs", TARGET, true, &["--target", TARGET]).unwrap_err(),
        "duplicate rustc invocation identity argument"
    );
}

#[test]
fn emitting_uses_explicit_units_and_matches_pinned_default_local_thin_lto() {
    let release = library(&[]);
    assert_eq!(release.units(), 16);
    assert!(release.codegen.local_thin_lto);
    let emitter = invocation_as(HARNESS, TARGET, true, &["-Ccodegen-units=16"]).unwrap();
    assert_eq!(emitter.compare(&release), Ok(()));
    assert_eq!(emitter.codegen.lto, Lto::Implicit);
    assert_eq!(
        invocation_as(HARNESS, TARGET, true, &[]).unwrap_err(),
        "assembly invocation has no explicit codegen units"
    );
    assert_eq!(
        invocation_as("ironafs_os", TARGET, false, &[]).unwrap_err(),
        "implicit codegen units are unverified for this compiler and target"
    );
    assert_eq!(
        invocation_as(HARNESS, TARGET, false, &["-Ccodegen-units=16"]).unwrap_err(),
        "implicit local ThinLTO is unverified for this compiler and target"
    );
    let emitter = invocation_as(HARNESS, TARGET, true, &["-Ccodegen-units=1"]).unwrap();
    assert_eq!(
        emitter.compare(&release).unwrap_err(),
        "ironafs_wipe_gate is not built under the shipping profile (different codegen-units, local ThinLTO)"
    );
}

#[test]
fn local_thin_lto_follows_optimisation_and_preserves_explicit_off() {
    assert!(!library(&["-Copt-level=0"]).codegen.local_thin_lto);
    assert!(library(&["-Copt-level=z"]).codegen.local_thin_lto);
    let release = library(&["-Clto=off"]);
    let emitter =
        invocation_as(HARNESS, TARGET, true, &["-Ccodegen-units=16", "-Clto=no"]).unwrap();
    assert_eq!(emitter.compare(&release), Ok(()));
    assert_eq!(emitter.codegen.lto, Lto::Off);
    assert!(!emitter.codegen.local_thin_lto);
}

#[test]
fn refuses_explicit_output_paths_that_can_force_a_single_unit() {
    for option in [
        &["-o", "/fresh build/a.s"][..],
        &["-o/fresh build/a.s"],
        &["--emit=asm=/fresh build/a.s"],
    ] {
        assert_eq!(
            invocation_as(HARNESS, TARGET, true, option).unwrap_err(),
            "explicit rustc output file can change the codegen-unit count"
        );
    }
}

#[test]
fn binary_refuses_unclassified_options_before_they_can_be_proof() {
    for option in ["-Zthinlto=no", "-Zinline-mir=no", "--print=cfg"] {
        assert_eq!(
            invocation_as("afs", TARGET, true, &[option, "unclassified"]).unwrap_err(),
            "unsupported rustc argument"
        );
    }
    for option in [
        "-Cpasses=none",
        "-Cllvm-args=x",
        "-Clink-arg=-flto",
        "-Clinker=cc",
        "-Coverflow-checks=0",
    ] {
        assert_eq!(
            invocation_as("afs", TARGET, true, &[option]).unwrap_err(),
            "unsupported rustc codegen option"
        );
    }
    assert_eq!(
        invocation_as("afs", TARGET, true, &["@/fresh build/flags"]).unwrap_err(),
        "rustc response files are unsupported"
    );
}

#[test]
fn binary_lto_requires_a_linkable_harness() {
    for flag in [
        "-Clto",
        "-Clto=thin",
        "-Clto=fat",
        "-Clto=unknown",
        "-Clto=",
        "-Clinker-plugin-lto=/plugin",
        "-Clinker_plugin_lto",
    ] {
        assert_eq!(
            invocation_as("afs", TARGET, true, &[flag]).unwrap_err(),
            super::UNSUPPORTED_LTO
        );
    }
}

#[test]
fn rejects_inputs_that_bypass_the_fresh_build() {
    for (args, expected) in [
        (
            ["--extern", "dep=/old/libdep.rlib"],
            "rustc extern is outside the fresh build",
        ),
        (
            ["-C", "incremental=/old/cache"],
            "rustc incremental cache is outside the fresh build",
        ),
        (
            ["-L", "dependency=/old/cache"],
            "unsupported rustc argument",
        ),
    ] {
        assert_eq!(
            invocation_as("afs", TARGET, true, &args).unwrap_err(),
            expected
        );
    }
}
