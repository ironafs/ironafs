// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! On-disk seam census controls and plants, using the normal checker.

use std::io::Write;
use std::path::Path;

use super::super::{CHECK, run_workspace};
use crate::fixture::{self, Fixture};
use crate::metadata::Workspace;
use crate::{Command, Error};

const CASE: &str = "written implementations in a Cargo workspace";
const IMPLEMENTATIONS: &[&str] = &[
    "check-seams: implementation ironafs-time::Clock | ironafs-time | test:double | crate | impl Clock for Double | crates/ironafs-time/tests/double.rs:3",
    "check-seams: implementation ironafs-time::Clock | outside-role | lib:outside_role | crate | impl Clock for Double | crates/outside-role/src/lib.rs:3",
    "check-seams: implementation ironafs-time::Clock | outside-role | lib:outside_role | crate | impl Clock for Second | crates/outside-role/src/lib.rs:5",
];
const DECLARATIONS: &[&str] = &[
    "check-seams: declaration ironafs-time::Clock | crates/ironafs-time/src/seam.rs:1",
    "check-seams: declaration ironafs-time::Rng | crates/ironafs-time/src/seam.rs:2",
    "check-seams: declaration ironafs-rx::Rpc | crates/ironafs-rx/src/rpc.rs:1",
    "check-seams: declaration ironafs-cache::Disk | crates/ironafs-cache/src/disk.rs:1",
];

pub(in crate::seams) fn run(output: &mut impl Write) -> Result<(), Error> {
    let dir = fixture::fresh_dir(&std::env::temp_dir(), "check-seams-census")?;
    let result = plants(&dir);
    let cleanup = fixture::remove_dir(&dir);
    result?;
    cleanup?;
    writeln!(output, "check-seams --self-test: verified exact source identities, addition, same-count substitution, and a missing seam home")
        .map_err(Error::io("report seam census self-test"))
}

fn plants(dir: &Path) -> Result<(), Error> {
    let fixture = census_fixture();
    let workspace = fixture.load(dir, CHECK, CASE)?;
    let compiled = Command::new("cargo")
        .current_dir(&workspace.root)
        .args([
            "check",
            "--offline",
            "--workspace",
            "--all-targets",
            "--manifest-path",
        ])
        .arg(workspace.root.join("Cargo.toml"))
        .output()
        .map_err(Error::io("compile seam census fixture"))?;
    if !compiled.status.success() {
        return Err(failure(
            "compiling the written implementations",
            &String::from_utf8_lossy(&compiled.stderr),
        ));
    }
    let output = census_output(&workspace)?;
    let declared: Vec<_> = output
        .lines()
        .filter(|line| line.starts_with("check-seams: declaration "))
        .collect();
    if implementation_rows(&output) != IMPLEMENTATIONS
        || declared != DECLARATIONS
        || !output
            .contains("check-seams: zero identified written implementations of ironafs-cache::Disk")
        || !output.contains("check-seams: written-source census resolved")
    {
        return Err(failure(
            "exact source identities with an empty seam",
            &output,
        ));
    }

    let file = workspace.root.join("crates/outside-role/src/lib.rs");
    let initial =
        std::fs::read_to_string(&file).map_err(Error::path_io("read fixture source", &file))?;
    fixture::write_file(
        &file,
        &format!("{initial}struct Third;\nimpl Clock for Third {{}}\n"),
    )?;
    let output = census_output(&workspace)?;
    let third = "check-seams: implementation ironafs-time::Clock | outside-role | lib:outside_role | crate | impl Clock for Third | crates/outside-role/src/lib.rs:7";
    if implementation_rows(&output) != [IMPLEMENTATIONS, &[third]].concat() {
        return Err(failure("adding an implementation", &output));
    }

    fixture::write_file(&file, &initial.replace("Second", "Updated"))?;
    let output = census_output(&workspace)?;
    let rows = implementation_rows(&output);
    let changed = "check-seams: implementation ironafs-time::Clock | outside-role | lib:outside_role | crate | impl Clock for Updated | crates/outside-role/src/lib.rs:5";
    if rows
        != [
            IMPLEMENTATIONS.first().copied().unwrap_or_default(),
            IMPLEMENTATIONS.get(1).copied().unwrap_or_default(),
            changed,
        ]
        || rows == IMPLEMENTATIONS
    {
        return Err(failure("same-count implementation substitution", &output));
    }

    let file = workspace.root.join("crates/ironafs-cache/src/disk.rs");
    fixture::write_file(&file, "pub struct Replacement;\n")?;
    fixture::write_file(
        &workspace.root.join("crates/ironafs-cache/src/lib.rs"),
        "mod disk;\npub use disk::Replacement;\n",
    )?;
    match census_output(&workspace) {
        Err(error)
            if error
                .to_string()
                .contains("missing declaration of ironafs-cache::Disk") =>
        {
            Ok(())
        }
        Err(error) => Err(failure("missing seam home", &error.to_string())),
        Ok(output) => Err(failure("missing seam home", &output)),
    }
}

fn failure(case: &'static str, output: &str) -> Error {
    Error::SelfTest {
        check: CHECK,
        case,
        found: output.lines().map(str::to_owned).collect(),
    }
}

pub(in crate::seams) fn census_output(workspace: &Workspace) -> Result<String, Error> {
    let mut output = Vec::new();
    run_workspace(workspace, &mut output)?;
    String::from_utf8(output).map_err(|_| Error::UnexpectedOutput(CHECK))
}

pub(in crate::seams) fn implementation_rows(output: &str) -> Vec<&str> {
    output
        .lines()
        .filter(|line| line.starts_with("check-seams: implementation "))
        .collect()
}

pub(in crate::seams) fn census_fixture() -> Fixture {
    let mut fixture = Fixture::default();
    for (krate, module, exports, declarations) in [
        (
            "ironafs-time",
            "seam",
            "{Clock, Rng}",
            "pub trait Clock {}\npub trait Rng {}\n",
        ),
        ("ironafs-rx", "rpc", "Rpc", "pub trait Rpc {}\n"),
        ("ironafs-cache", "disk", "Disk", "pub trait Disk {}\n"),
    ] {
        fixture.add_crate(krate, &[]);
        fixture.file(
            krate,
            "src/lib.rs",
            Some(&format!("mod {module};\npub use {module}::{exports};\n")),
        );
        fixture.file(krate, &format!("src/{module}.rs"), Some(declarations));
    }
    fixture.file(
        "ironafs-time",
        "tests/double.rs",
        Some("use ironafs_time::Clock;\nstruct Double;\nimpl Clock for Double {}\n"),
    );
    fixture.add_crate("outside-role", &["ironafs-time"]);
    fixture.file(
        "outside-role",
        "src/lib.rs",
        Some("use ironafs_time::Clock;\nstruct Double;\nimpl Clock for Double {}\nstruct Second;\nimpl Clock for Second {}\n"),
    );
    fixture
}
