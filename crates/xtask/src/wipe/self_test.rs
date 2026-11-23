// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! `check-wipe --self-test`: runs the gate end to end, through the same
//! harness crate, builds, assembly search and profile comparison as the
//! check, first on the clean harness, which must pass, then on each planted
//! regression, which must fail on every target the gate runs on here with
//! what was planted: `wipe` not inlined, too few zero stores, a control
//! whose stores survive, and a harness built under another profile.

use std::io::Write;
use std::path::Path;

use super::{CHECK, HARNESS_SOURCE, check_targets, installed_targets};
use crate::Error;
use crate::metadata::Workspace;

/// One planted regression: the harness source and profile, and what each
/// target's failure must say.
pub(super) struct Case {
    pub(super) name: &'static str,
    pub(super) source: String,
    pub(super) profile: String,
    pub(super) says: &'static str,
}

/// The cases, from the clean harness and `profile`.
pub(super) fn cases(profile: &str) -> Vec<Case> {
    let plant = |from: &str, to: &str| HARNESS_SOURCE.replacen(from, to, 1);
    vec![
        Case {
            name: "wipe called through a pointer, not inlined",
            source: plant(
                "let mut buffer = [fill; 64];\nironafs_os::wipe(&mut buffer);\n}",
                "let mut buffer = [fill; 64];\n\
                 let wipe: fn(&mut [u8]) = core::hint::black_box(ironafs_os::wipe);\n\
                 wipe(&mut buffer);\n}",
            ),
            profile: profile.to_owned(),
            says: "refers to ironafs-os, so `wipe` is not inlined there",
        },
        Case {
            name: "half the buffer wiped",
            source: plant(
                "ironafs_os::wipe(&mut buffer);\nbuffer.iter()",
                "ironafs_os::wipe(buffer.get_mut(..32).unwrap_or_default());\nbuffer.iter()",
            ),
            profile: profile.to_owned(),
            says: "zero byte stores for a buffer of 64 bytes",
        },
        Case {
            name: "the control's stores kept",
            source: plant(
                "core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);\n}",
                "core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);\n\
                 core::hint::black_box(&buffer);\n}",
            ),
            profile: profile.to_owned(),
            says: "the control `plain_dead`",
        },
        Case {
            // A key the workspace's release table does not set; if it ever
            // does, the harness fails to build and this case says so.
            name: "the harness built under another profile",
            source: HARNESS_SOURCE.to_owned(),
            profile: format!("{}\ncodegen-units = 7\n", profile.trim_end()),
            says: "is not built under the shipping profile",
        },
    ]
}

/// Whether `violations` hold, for every one of `targets`, a failure of that
/// target that says `says`.
pub(super) fn as_planted(violations: &[String], targets: &[&str], says: &str) -> bool {
    targets.iter().all(|target| {
        violations.iter().any(|violation| {
            violation.starts_with(&format!("{target}: ")) && violation.contains(says)
        })
    })
}

/// Runs the clean case and every planted case.
///
/// # Errors
///
/// Returns the first case that did not end as it should, or why the gate
/// could not run.
pub(super) fn run(
    workspace: &Workspace,
    crate_dir: &Path,
    profile: &str,
    output: &mut impl Write,
) -> Result<(), Error> {
    let targets = installed_targets(workspace)?;
    if targets.is_empty() {
        return Err(failure(
            "the clean harness",
            &["no Linux target's standard library is installed: nothing ran".to_owned()],
        ));
    }
    let dir = crate::fixture::fresh_dir(&workspace.target_dir, CHECK)?;
    let result = run_in(workspace, crate_dir, profile, &targets, &dir, output);
    crate::fixture::remove_dir(&dir)?;
    result
}

fn run_in(
    workspace: &Workspace,
    crate_dir: &Path,
    profile: &str,
    targets: &[&str],
    dir: &Path,
    output: &mut impl Write,
) -> Result<(), Error> {
    let mut log = Vec::new();
    match check_targets(
        workspace,
        crate_dir,
        profile,
        HARNESS_SOURCE,
        &dir.join("clean"),
        &mut log,
    ) {
        Ok(()) => say(output, "passed: the clean harness")?,
        Err(Error::Rules { violations, .. }) => {
            return Err(failure("the clean harness", &violations));
        }
        Err(error) => return Err(error),
    }
    for (index, case) in cases(profile).into_iter().enumerate() {
        let mut log = Vec::new();
        let found = check_targets(
            workspace,
            crate_dir,
            &case.profile,
            &case.source,
            &dir.join(format!("case-{index}")),
            &mut log,
        );
        match found {
            Err(Error::Rules { violations, .. }) if as_planted(&violations, targets, case.says) => {
                say(
                    output,
                    &format!("failed as planted on {}: {}", targets.join(", "), case.name),
                )?;
            }
            Err(Error::Rules { violations, .. }) => return Err(failure(case.name, &violations)),
            Ok(()) => return Err(failure(case.name, &[])),
            Err(error) => return Err(error),
        }
    }
    say(
        output,
        "check-wipe --self-test: the clean harness passed and every planted case failed as \
         planted",
    )
}

fn say(output: &mut impl Write, line: &str) -> Result<(), Error> {
    writeln!(output, "{line}").map_err(Error::io("report check-wipe --self-test"))
}

fn failure(case: &'static str, found: &[String]) -> Error {
    Error::SelfTest {
        check: CHECK,
        case,
        found: found.to_vec(),
    }
}
