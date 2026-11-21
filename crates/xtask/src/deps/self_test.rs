// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! `check-deps --self-test`: plants each forbidden edge and each forbidden
//! standard-library path in a fixture workspace and proves the check fails
//! on it, and passes on the clean fixture.

use std::io::Write;
use std::path::{Path, PathBuf};

use super::{Rule, Violation, check, graph_violations};
use crate::Error;
use crate::fixture::{self, Fixture};
use crate::metadata::Workspace;

const CHECK: &str = "check-deps";

/// A workspace the rules allow, with one crate of each kind, a development
/// dependency on a test-only crate, and a pure crate that names allowed
/// standard-library paths.
fn clean() -> Fixture {
    let mut fixture = Fixture::default();
    for (name, dependencies) in [
        ("ironafs-time", &[][..]),
        ("ironafs-xdr", &[]),
        ("ironafs-crypto", &[]),
        (
            "ironafs-rx",
            &["ironafs-xdr", "ironafs-time", "ironafs-crypto"],
        ),
        ("ironafs-rxkad", &["ironafs-rx", "ironafs-crypto"]),
        (
            "ironafs-proto",
            &["ironafs-rx", "ironafs-xdr", "ironafs-time"],
        ),
        ("ironafs-vfs", &["ironafs-time"]),
        (
            "ironafs-cache",
            &["ironafs-proto", "ironafs-vfs", "ironafs-time"],
        ),
        ("ironafs-ipc", &["ironafs-vfs"]),
        ("ironafs-events", &[]),
        ("ironafs-auth", &["ironafs-time"]),
        ("ironafs-config", &[]),
        (
            "ironafs-client",
            &[
                "ironafs-cache",
                "ironafs-proto",
                "ironafs-rx",
                "ironafs-vfs",
                "ironafs-events",
            ],
        ),
        ("ironafs-os", &["ironafs-time"]),
        ("ironafs-net", &["ironafs-rx", "ironafs-os"]),
        (
            "ironafs-fuse",
            &[
                "ironafs-vfs",
                "ironafs-events",
                "ironafs-ipc",
                "ironafs-os",
                "ironafs-crypto",
            ],
        ),
        (
            "ironafsd",
            &[
                "ironafs-client",
                "ironafs-net",
                "ironafs-fuse",
                "ironafs-os",
            ],
        ),
        ("ironafs-test-support", &[]),
        ("ironafs-sim", &["ironafs-client", "ironafs-time"]),
        ("xtask", &["ironafs-sim"]),
    ] {
        fixture.add_crate(name, dependencies);
    }
    fixture.manifest_tail(
        "ironafs-cache",
        "\n[dev-dependencies]\nironafs-sim = { path = \"../ironafs-sim\" }\n\
         ironafs-test-support = { path = \"../ironafs-test-support\" }\n",
    );
    fixture.file(
        "ironafs-proto",
        "src/lib.rs",
        Some(
            "//! Uses std::net only in this comment.\n\
             mod codec;\n#[cfg(test)]\nmod tests;\n\
             use core::net::Ipv4Addr;\nuse std::time::Duration;\n\
             pub fn f(_: Ipv4Addr, _: Duration) -> &'static str { \"std::fs\" }\n",
        ),
    );
    fixture.file(
        "ironafs-proto",
        "src/codec.rs",
        Some("use std::io::Read;\n"),
    );
    fixture.file(
        "ironafs-proto",
        "src/tests.rs",
        Some("use std::fs;\nuse std::time::Instant;\n"),
    );
    fixture
}

fn plant_source(fixture: &mut Fixture, source: &str) {
    fixture.file("ironafs-proto", "src/codec.rs", Some(source));
}

fn is_edge(violation: &Violation, from: &str, to: &str, rule: Rule) -> bool {
    matches!(violation, Violation::Edge { from: found_from, to: found_to, rule: found_rule, .. }
        if found_from == from && found_to == to && *found_rule == rule)
}

/// Whether the violation is a planted path at or below `tree`.
fn is_std_path(violation: &Violation, tree: &str) -> bool {
    matches!(violation, Violation::ForbiddenStdPath { file, path, .. }
        if file == Path::new("crates/ironafs-proto/src/codec.rs")
            && path.strip_prefix(tree).is_some_and(|rest| rest.is_empty() || rest.starts_with("::")))
}

/// A planted case: what it plants, and what every violation it produces
/// must match. At least one violation is required.
struct Case {
    name: &'static str,
    plant: fn(&mut Fixture),
    expect: fn(&Violation) -> bool,
}

const CASES: &[Case] = &[
    Case {
        name: "an unclassified workspace crate",
        plant: |fixture| fixture.add_crate("ironafs-new", &[]),
        expect: |found| matches!(found, Violation::Unclassified(name) if name == "ironafs-new"),
    },
    Case {
        name: "a pure crate on a driver",
        plant: |fixture| fixture.depend("ironafs-auth", "ironafs-os"),
        expect: |found| is_edge(found, "ironafs-auth", "ironafs-os", Rule::PureOnNonPure),
    },
    Case {
        name: "a pure crate on ironafs-client",
        plant: |fixture| fixture.depend("ironafs-config", "ironafs-client"),
        expect: |found| {
            is_edge(
                found,
                "ironafs-config",
                "ironafs-client",
                Rule::PureOnNonPure,
            )
        },
    },
    Case {
        name: "a layer crate on ironafs-client",
        plant: |fixture| fixture.depend("ironafs-fuse", "ironafs-client"),
        // ironafs-client brings ironafs-cache with it: both are reported.
        expect: |found| {
            is_edge(
                found,
                "ironafs-fuse",
                "ironafs-client",
                Rule::LayerBypassesCore,
            ) || matches!(found, Violation::Edge { from, via, to, rule: Rule::LayerBypassesCore }
                    if from == "ironafs-fuse" && to == "ironafs-cache" && via == &["ironafs-client"])
        },
    },
    Case {
        name: "a layer crate reaching ironafs-cache through a pure crate",
        plant: |fixture| fixture.depend("ironafs-ipc", "ironafs-cache"),
        expect: |found| {
            matches!(found, Violation::Edge { from, via, to, rule: Rule::LayerBypassesCore }
                if from == "ironafs-fuse" && to == "ironafs-cache" && via == &["ironafs-ipc"])
        },
    },
    Case {
        name: "a layer crate on ironafs-cache under another name",
        plant: |fixture| {
            fixture.dependency_line(
                "ironafs-fuse",
                "state = { package = \"ironafs-cache\", path = \"../ironafs-cache\" }",
            );
        },
        expect: |found| {
            is_edge(
                found,
                "ironafs-fuse",
                "ironafs-cache",
                Rule::LayerBypassesCore,
            )
        },
    },
    Case {
        name: "ironafs-rx on an AFS crate",
        plant: |fixture| fixture.depend("ironafs-rx", "ironafs-events"),
        expect: |found| is_edge(found, "ironafs-rx", "ironafs-events", Rule::RxKnowsAfs),
    },
    Case {
        name: "a protocol crate on a client-side crate",
        plant: |fixture| fixture.depend("ironafs-proto", "ironafs-vfs"),
        expect: |found| {
            is_edge(
                found,
                "ironafs-proto",
                "ironafs-vfs",
                Rule::ProtocolOnClientSide,
            )
        },
    },
    Case {
        name: "a shipped crate on ironafs-sim",
        plant: |fixture| fixture.depend("ironafsd", "ironafs-sim"),
        expect: |found| is_edge(found, "ironafsd", "ironafs-sim", Rule::ShippedOnTestOnly),
    },
    Case {
        name: "a build dependency on ironafs-test-support",
        plant: |fixture| {
            fixture.manifest_tail(
                "ironafsd",
                "\n[build-dependencies]\nironafs-test-support = { path = \"../ironafs-test-support\" }\n",
            );
        },
        expect: |found| {
            is_edge(
                found,
                "ironafsd",
                "ironafs-test-support",
                Rule::ShippedOnTestOnly,
            )
        },
    },
    Case {
        name: "an edge behind an optional feature",
        plant: |fixture| {
            fixture.dependency_line(
                "ironafs-config",
                "ironafs-os = { path = \"../ironafs-os\", optional = true }",
            );
            fixture.manifest_tail(
                "ironafs-config",
                "\n[features]\nhost = [\"dep:ironafs-os\"]\n",
            );
        },
        expect: |found| is_edge(found, "ironafs-config", "ironafs-os", Rule::PureOnNonPure),
    },
    Case {
        name: "an edge on another platform only",
        plant: |fixture| {
            fixture.manifest_tail(
                "ironafs-events",
                "\n[target.'cfg(windows)'.dependencies]\nironafs-os = { path = \"../ironafs-os\" }\n",
            );
        },
        expect: |found| is_edge(found, "ironafs-events", "ironafs-os", Rule::PureOnNonPure),
    },
    Case {
        name: "ironafs-time with a dependency",
        plant: |fixture| fixture.depend("ironafs-time", "ironafs-crypto"),
        expect: |found| matches!(found, Violation::TimeHasDependency(name) if name == "ironafs-crypto"),
    },
    Case {
        name: "a pure crate with no library target",
        plant: |fixture| {
            fixture.file("ironafs-config", "src/lib.rs", None);
            fixture.file("ironafs-config", "src/main.rs", Some("fn main() {}\n"));
        },
        expect: |found| matches!(found, Violation::NoLibrary(name) if name == "ironafs-config"),
    },
    Case {
        name: "a std::net type in a pure crate",
        plant: |fixture| {
            plant_source(
                fixture,
                "fn f() { let _ = std::net::UdpSocket::bind(a); }\n",
            );
        },
        expect: |found| is_std_path(found, "std::net::UdpSocket::bind"),
    },
    Case {
        name: "std::fs imported by a pure crate",
        plant: |fixture| plant_source(fixture, "use std::fs;\n"),
        expect: |found| is_std_path(found, "std::fs"),
    },
    Case {
        name: "std::thread in a grouped import",
        plant: |fixture| {
            plant_source(
                fixture,
                "use std::{io::Read, thread::{self, JoinHandle}};\n",
            );
        },
        expect: |found| is_std_path(found, "std::thread"),
    },
    Case {
        name: "the monotonic clock read directly",
        plant: |fixture| plant_source(fixture, "fn now() { std::time::Instant::now(); }\n"),
        expect: |found| is_std_path(found, "std::time::Instant::now"),
    },
    Case {
        name: "the wall clock imported beside Duration",
        plant: |fixture| plant_source(fixture, "use std::time::{Duration, SystemTime};\n"),
        expect: |found| is_std_path(found, "std::time::SystemTime"),
    },
    Case {
        name: "a glob of std::time",
        plant: |fixture| plant_source(fixture, "use std::time::*;\n"),
        expect: |found| is_std_path(found, "std::time::*"),
    },
    Case {
        name: "the standard library renamed",
        plant: |fixture| plant_source(fixture, "use std as host;\n"),
        expect: |found| is_std_path(found, "std"),
    },
];

/// Runs the clean fixture, every planted case, and the empty workspace.
///
/// # Errors
///
/// Returns the first case that did not end as planted.
pub(super) fn run(target_dir: &Path, output: &mut impl Write) -> Result<(), Error> {
    let dir = fixture::fresh_dir(target_dir, CHECK)?;
    let result = run_in(&dir, output);
    fixture::remove_dir(&dir)?;
    result
}

fn run_in(dir: &Path, output: &mut impl Write) -> Result<(), Error> {
    let report = |output: &mut dyn Write, line: String| {
        writeln!(output, "{line}").map_err(Error::io("report check-deps self-test"))
    };

    let (found, mut scanned) = check_fixture(&clean(), &dir.join("clean"), "the clean fixture")?;
    scanned.sort();
    if !found.is_empty() || scanned != clean_pure_files() {
        return Err(failure("the clean fixture", &found));
    }
    report(
        output,
        format!(
            "passed: the clean fixture ({} pure-crate files read)",
            scanned.len()
        ),
    )?;

    for (index, case) in CASES.iter().enumerate() {
        let mut planted = clean();
        (case.plant)(&mut planted);
        let (found, _) = check_fixture(&planted, &dir.join(format!("case-{index}")), case.name)?;
        match found.first() {
            Some(first) if found.iter().all(case.expect) => {
                report(output, format!("caught: {}: {first}", case.name))?;
            }
            _ => return Err(failure(case.name, &found)),
        }
    }

    let empty = Workspace {
        root: dir.to_path_buf(),
        target_dir: dir.to_path_buf(),
        members: Vec::new(),
    };
    let found = graph_violations(&empty);
    if found != [Violation::EmptyWorkspace] {
        return Err(failure("an empty workspace", &found));
    }
    report(output, "caught: an empty workspace".to_owned())?;
    report(
        output,
        format!(
            "check-deps --self-test: the clean fixture passed and {} planted cases failed as planted",
            CASES.len() + 1
        ),
    )
}

/// The files the scan must read in the clean fixture, sorted: every pure
/// crate's library root and the module beside one, but no unit-test file.
fn clean_pure_files() -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = clean()
        .names()
        .into_iter()
        .filter(|name| super::kind_of(name) == Some(super::Kind::Pure))
        .map(|name| PathBuf::from(format!("crates/{name}/src/lib.rs")))
        .chain([PathBuf::from("crates/ironafs-proto/src/codec.rs")])
        .collect();
    files.sort();
    files
}

fn failure(case: &'static str, found: &[Violation]) -> Error {
    Error::SelfTest {
        check: CHECK,
        case,
        found: found.iter().map(ToString::to_string).collect(),
    }
}

/// Writes a fixture, reads it back through `cargo metadata`, confirms every
/// crate was read, and runs the check on it.
fn check_fixture(
    fixture: &Fixture,
    dir: &Path,
    case: &'static str,
) -> Result<(Vec<Violation>, Vec<PathBuf>), Error> {
    let workspace = fixture.load(dir, CHECK, case)?;
    check(&workspace)
}
