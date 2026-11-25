// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Real Cargo binary probes, including release profiles that need linking.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::Error;
use crate::fixture::Fixture;

use super::{Tools, failure, say};
use crate::wipe::capture::{Stage, check_externs, refusal};
use crate::wipe::options::Invocation;

const DEP: &str = "fixture-dep";
const CLI: &str = "ironafs-cli";

fn fixture(dir: &Path, config: &str, dependency: Option<&str>) -> Result<PathBuf, Error> {
    let mut fixture = Fixture::default();
    if let Some(source) = dependency {
        fixture.add_crate(DEP, &[]);
        fixture.file(DEP, "src/lib.rs", Some(source));
        fixture.add_crate(CLI, &[DEP]);
    } else {
        fixture.add_crate(CLI, &[]);
    }
    fixture.file(CLI, "src/lib.rs", None);
    fixture.file(CLI, "src/main.rs", Some("fn main() {}\n"));
    fixture.manifest_tail(CLI, "\n[[bin]]\nname = \"afs\"\npath = \"src/main.rs\"\n");
    fixture.root_file(
        ".cargo/config.toml",
        &format!(
            "[build]\nrustc-wrapper = \"./missing-outer-wrapper\"\n\
             rustc-workspace-wrapper = \"./missing-workspace-wrapper\"\n{config}"
        ),
    );
    let manifest = fixture.write(dir)?;
    manifest
        .parent()
        .map(Path::to_path_buf)
        .ok_or(Error::UnexpectedOutput("binary fixture manifest directory"))
}

fn probe(
    tools: &Tools,
    dir: &Path,
    target: &str,
    config: &str,
    dependency: Option<&str>,
    flags: Option<&str>,
) -> Result<BTreeMap<String, Invocation>, Error> {
    let root = fixture(dir, config, dependency)?;
    let invocations = tools.build(&root, dir, target, Stage::Binary, |command| {
        command.args(["-p", CLI, "--bin", "afs"]);
        if let Some(flags) = flags {
            command.env("CARGO_ENCODED_RUSTFLAGS", flags);
        }
    })?;
    check_externs(&invocations)?;
    Ok(invocations)
}

pub(super) fn run(
    tools: &Tools,
    dir: &Path,
    target: &str,
    output: &mut impl Write,
) -> Result<(), Error> {
    let dependency = Some("pub fn f() {}\n");
    let default = probe(tools, &dir.join("default"), target, "", dependency, None)?;
    let Some(binary) = default.get("afs") else {
        return Err(failure("default binary probe", &["missing afs".to_owned()]));
    };
    let Some(lib) = default.get("fixture_dep") else {
        return Err(failure(
            "default binary probe",
            &["missing target dependency".to_owned()],
        ));
    };
    lib.compare(binary).map_err(refusal)?;
    say(
        output,
        &format!(
            "{target}: passed: default afs and target dependency captured with both Cargo wrappers configured"
        ),
    )?;

    for (case, config) in [
        (
            "cross-crate thin LTO",
            "[profile.release]\nlto = \"thin\"\n",
        ),
        ("cross-crate fat LTO", "[profile.release]\nlto = \"fat\"\n"),
    ] {
        expect_refusal(
            case,
            probe(tools, &dir.join(case), target, config, dependency, None),
            "cross-crate or unrecognised LTO requires a linkable wipe harness",
            output,
        )?;
    }
    expect_refusal(
        "dependency error before the binary marker",
        probe(
            tools,
            &dir.join("broken"),
            target,
            "",
            Some("compile_error!(\"fixture dependency failed\");\n"),
            None,
        ),
        "Cargo did not end solely at the expected afs binary marker",
        output,
    )?;
    expect_refusal(
        "opaque flag in the binary",
        probe(
            tools,
            &dir.join("opaque"),
            target,
            "",
            None,
            Some("-Cllvm-args=--inline-threshold=1"),
        ),
        "unsupported rustc codegen option",
        output,
    )?;

    let overridden = probe(
        tools,
        &dir.join("override"),
        target,
        "[profile.release.package.fixture-dep]\ncodegen-units = 7\n",
        dependency,
        None,
    )?;
    let Some(dep) = overridden.get("fixture_dep") else {
        return Err(failure(
            "target dependency override",
            &["missing target dependency".to_owned()],
        ));
    };
    expect_refusal(
        "target dependency profile override",
        dep.compare(binary).map_err(refusal),
        "fixture_dep is not built under the shipping profile (different codegen-units)",
        output,
    )
}

fn expect_refusal<T>(
    case: &'static str,
    result: Result<T, Error>,
    expected: &str,
    output: &mut impl Write,
) -> Result<(), Error> {
    match result {
        Err(Error::Rules { violations, .. }) if violations == [expected] => {
            say(output, &format!("failed as planted: {case} ({expected})"))
        }
        Err(Error::Rules { violations, .. }) => Err(failure(case, &violations)),
        Ok(_) => Err(failure(case, &[])),
        Err(error) => Err(error),
    }
}
