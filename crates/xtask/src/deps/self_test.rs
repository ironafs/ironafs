// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! `check-deps --self-test`: plants each forbidden edge and each forbidden
//! name in a fixture workspace and proves the check fails on it with what
//! was planted, and passes on the clean fixture.

use std::io::Write;
use std::path::{Path, PathBuf};

use super::{Rule, Violation, check, graph_violations};
use crate::Error;
use crate::fixture::{self, Fixture};
use crate::metadata::Workspace;

const CHECK: &str = "check-deps";

/// A workspace the rules allow, with one crate of each kind, a development
/// dependency on a test-only crate, a foreign-function crate in
/// `ironafs-os`, pure crates that name allowed standard-library paths and
/// gated unit-test modules that name forbidden ones, and an
/// `ironafs-client` that uses threads.
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
        ("ironafs-os", &["ironafs-time", "libc"]),
        ("ironafs-net", &["ironafs-rx", "ironafs-os"]),
        (
            "ironafs-fuse",
            &[
                "ironafs-vfs",
                "ironafs-events",
                "ironafs-ipc",
                "ironafs-os",
                "ironafs-crypto",
                "ironafs-time",
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
    fixture.add_third_party("libc", &[]);
    fixture.manifest_tail(
        "ironafs-cache",
        "\n[dev-dependencies]\nironafs-sim = { path = \"../ironafs-sim\" }\n\
         ironafs-test-support = { path = \"../ironafs-test-support\" }\n",
    );
    clean_sources(&mut fixture);
    fixture
}

/// The clean fixture's source files.
fn clean_sources(fixture: &mut Fixture) {
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
        Some(
            "use std::io::Read;\nuse std::collections::BTreeMap;\n\
             #[cfg(test)]\nmod tests;\n\
             #[derive(Debug)]\npub struct Entry { pub metadata: u32 }\n\
             pub fn f(e: &Entry) -> u32 { e.metadata }\n",
        ),
    );
    fixture.file(
        "ironafs-proto",
        "src/tests.rs",
        Some("use std::fs;\nuse std::time::Instant;\n"),
    );
    fixture.file(
        "ironafs-proto",
        "src/codec/tests.rs",
        Some("mod helper;\nuse std::net::UdpSocket;\n"),
    );
    fixture.file(
        "ironafs-proto",
        "src/codec/tests/helper.rs",
        Some("fn f() { std::thread::sleep(d); println!(\"x\"); }\n"),
    );
    fixture.file(
        "ironafs-client",
        "src/lib.rs",
        Some(
            "use std::thread::{self, JoinHandle};\n\
             pub fn run(f: fn()) -> JoinHandle<()> { thread::spawn(f) }\n",
        ),
    );
}

/// The crate and file a planted source case writes.
const PLANTED_CRATE: &str = "ironafs-proto";
const PLANTED: &str = "crates/ironafs-proto/src/planted.rs";
const CLIENT: &str = "crates/ironafs-client/src/lib.rs";

fn plant_source(fixture: &mut Fixture, source: &str) {
    fixture.file(PLANTED_CRATE, "src/planted.rs", Some(source));
}

fn is_edge(violation: &Violation, from: &str, to: &str, rule: Rule) -> bool {
    matches!(violation, Violation::Edge { from: found_from, to: found_to, rule: found_rule, .. }
        if found_from == from && found_to == to && *found_rule == rule)
}

/// Whether the violation is a forbidden name in `file`, at or below `name`.
fn is_source(violation: &Violation, in_file: &str, name: &str) -> bool {
    matches!(violation, Violation::ForbiddenSource { file, name: found, .. }
        if file == Path::new(in_file)
            && found.strip_prefix(name).is_some_and(|rest| rest.is_empty() || rest.starts_with("::")))
}

fn is_planted(violation: &Violation, name: &str) -> bool {
    is_source(violation, PLANTED, name)
}

/// A planted case: what it plants, and what it must produce. Each
/// predicate must match at least one violation, and each violation at
/// least one predicate.
struct Case {
    name: &'static str,
    plant: fn(&mut Fixture),
    expect: &'static [fn(&Violation) -> bool],
}

const CASES: &[Case] = &[
    Case {
        name: "an unclassified workspace crate",
        plant: |fixture| fixture.add_crate("ironafs-new", &[]),
        expect: &[|found| matches!(found, Violation::Unclassified(name) if name == "ironafs-new")],
    },
    Case {
        name: "a pure crate on a driver",
        plant: |fixture| fixture.depend("ironafs-auth", "ironafs-os"),
        expect: &[|found| is_edge(found, "ironafs-auth", "ironafs-os", Rule::PureOnNonPure)],
    },
    Case {
        name: "a pure crate on ironafs-client",
        plant: |fixture| fixture.depend("ironafs-config", "ironafs-client"),
        expect: &[|found| {
            is_edge(
                found,
                "ironafs-config",
                "ironafs-client",
                Rule::PureOnNonPure,
            )
        }],
    },
    Case {
        name: "a layer crate on ironafs-client",
        plant: |fixture| fixture.depend("ironafs-fuse", "ironafs-client"),
        // ironafs-client brings ironafs-cache with it: both are reported.
        expect: &[
            |found| {
                is_edge(
                    found,
                    "ironafs-fuse",
                    "ironafs-client",
                    Rule::LayerBypassesCore,
                )
            },
            |found| {
                matches!(found, Violation::Edge { from, via, to, rule: Rule::LayerBypassesCore }
                    if from == "ironafs-fuse" && to == "ironafs-cache" && via == &["ironafs-client"])
            },
        ],
    },
    Case {
        name: "a layer crate reaching ironafs-cache through a pure crate",
        plant: |fixture| fixture.depend("ironafs-ipc", "ironafs-cache"),
        expect: &[|found| {
            matches!(found, Violation::Edge { from, via, to, rule: Rule::LayerBypassesCore }
                if from == "ironafs-fuse" && to == "ironafs-cache" && via == &["ironafs-ipc"])
        }],
    },
    Case {
        name: "a layer crate on ironafs-cache under another name",
        plant: |fixture| {
            fixture.dependency_line(
                "ironafs-fuse",
                "state = { package = \"ironafs-cache\", path = \"../ironafs-cache\" }",
            );
        },
        expect: &[|found| {
            is_edge(
                found,
                "ironafs-fuse",
                "ironafs-cache",
                Rule::LayerBypassesCore,
            )
        }],
    },
    Case {
        name: "a layer crate on a workspace crate outside its list",
        plant: |fixture| fixture.depend("ironafs-fuse", "ironafs-auth"),
        expect: &[|found| {
            is_edge(
                found,
                "ironafs-fuse",
                "ironafs-auth",
                Rule::LayerOutsideList,
            )
        }],
    },
    Case {
        name: "ironafs-rx on an AFS crate",
        plant: |fixture| fixture.depend("ironafs-rx", "ironafs-events"),
        expect: &[|found| is_edge(found, "ironafs-rx", "ironafs-events", Rule::RxKnowsAfs)],
    },
    Case {
        name: "a protocol crate on a client-side crate",
        plant: |fixture| fixture.depend("ironafs-proto", "ironafs-vfs"),
        expect: &[|found| {
            is_edge(
                found,
                "ironafs-proto",
                "ironafs-vfs",
                Rule::ProtocolOnClientSide,
            )
        }],
    },
    Case {
        name: "ironafs-client on a driver",
        plant: |fixture| fixture.depend("ironafs-client", "ironafs-net"),
        // ironafs-net brings ironafs-os with it: both are reported.
        expect: &[
            |found| is_edge(found, "ironafs-client", "ironafs-net", Rule::CoreOnDriver),
            |found| is_edge(found, "ironafs-client", "ironafs-os", Rule::CoreOnDriver),
        ],
    },
    Case {
        name: "a shipped crate on ironafs-sim",
        plant: |fixture| fixture.depend("ironafsd", "ironafs-sim"),
        expect: &[|found| is_edge(found, "ironafsd", "ironafs-sim", Rule::ShippedOnTestOnly)],
    },
    Case {
        name: "a build dependency on ironafs-test-support",
        plant: |fixture| {
            fixture.manifest_tail(
                "ironafsd",
                "\n[build-dependencies]\nironafs-test-support = { path = \"../ironafs-test-support\" }\n",
            );
        },
        expect: &[|found| {
            is_edge(
                found,
                "ironafsd",
                "ironafs-test-support",
                Rule::ShippedOnTestOnly,
            )
        }],
    },
    Case {
        name: "a shipped crate on xtask",
        plant: |fixture| fixture.depend("ironafsd", "xtask"),
        // xtask brings ironafs-sim with it: both are reported.
        expect: &[
            |found| is_edge(found, "ironafsd", "xtask", Rule::OnAutomation),
            |found| is_edge(found, "ironafsd", "ironafs-sim", Rule::ShippedOnTestOnly),
        ],
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
        expect: &[|found| is_edge(found, "ironafs-config", "ironafs-os", Rule::PureOnNonPure)],
    },
    Case {
        name: "an edge on another platform only",
        plant: |fixture| {
            fixture.manifest_tail(
                "ironafs-auth",
                "\n[target.'cfg(windows)'.dependencies]\nironafs-os = { path = \"../ironafs-os\" }\n",
            );
        },
        expect: &[|found| is_edge(found, "ironafs-auth", "ironafs-os", Rule::PureOnNonPure)],
    },
    Case {
        name: "a local crate outside the workspace between a pure crate and a driver",
        plant: |fixture| {
            fixture.add_outside_crate("helper", &["ironafs-net"]);
            fixture.depend("ironafs-config", "helper");
        },
        // ironafs-net brings ironafs-os with it: both are reported.
        expect: &[
            |found| matches!(found, Violation::NotAMember { name, .. } if name == "helper"),
            |found| {
                matches!(found, Violation::ThirdParty { from, to }
                    if from == "ironafs-config" && to == "helper")
            },
            |found| {
                matches!(found, Violation::Edge { from, via, to, rule: Rule::PureOnNonPure }
                    if from == "ironafs-config" && to == "ironafs-net" && via == &["helper"])
            },
            |found| {
                matches!(found, Violation::Edge { from, via, to, rule: Rule::PureOnNonPure }
                    if from == "ironafs-config" && to == "ironafs-os" && via == &["helper", "ironafs-net"])
            },
        ],
    },
    Case {
        name: "a workspace crate outside crates/",
        plant: |fixture| {
            fixture.add_crate_at("tools/ironafs-cloudfiles", "ironafs-cloudfiles", &[]);
            fixture.depend("ironafsd", "ironafs-cloudfiles");
        },
        expect: &[
            |found| matches!(found, Violation::OutsideCrates { name, .. } if name == "ironafs-cloudfiles"),
        ],
    },
    Case {
        name: "a pure crate on a third-party crate",
        plant: |fixture| {
            fixture.add_third_party("bytes", &[]);
            fixture.depend("ironafs-xdr", "bytes");
        },
        expect: &[
            |found| matches!(found, Violation::ThirdParty { from, to } if from == "ironafs-xdr" && to == "bytes"),
        ],
    },
    Case {
        name: "a layer crate on a third-party crate",
        plant: |fixture| {
            fixture.add_third_party("fuser", &[]);
            fixture.depend("ironafs-fuse", "fuser");
        },
        expect: &[
            |found| matches!(found, Violation::ThirdParty { from, to } if from == "ironafs-fuse" && to == "fuser"),
        ],
    },
    Case {
        name: "a cryptography crate outside ironafs-crypto and ironafs-os",
        plant: |fixture| {
            fixture.add_third_party("ring", &[]);
            fixture.depend("ironafs-client", "ring");
        },
        expect: &[|found| {
            matches!(found, Violation::CryptoOutsideHome { from, via, to }
                if from == "ironafs-client" && via.is_empty() && to == "ring")
        }],
    },
    Case {
        name: "a foreign-function crate reached through another third-party crate",
        plant: |fixture| {
            fixture.add_third_party("wrapper", &["libc"]);
            fixture.depend("ironafsd", "wrapper");
        },
        expect: &[|found| {
            matches!(found, Violation::CryptoOutsideHome { from, via, to }
                if from == "ironafsd" && via == &["wrapper"] && to == "libc")
        }],
    },
    Case {
        name: "ironafs-time with a dependency",
        plant: |fixture| fixture.depend("ironafs-time", "ironafs-crypto"),
        expect: &[|found| {
            matches!(found, Violation::HasDependency { name, dependency }
                if name == "ironafs-time" && dependency == "ironafs-crypto")
        }],
    },
    Case {
        name: "ironafs-events with a dependency",
        plant: |fixture| fixture.depend("ironafs-events", "ironafs-time"),
        expect: &[|found| {
            matches!(found, Violation::HasDependency { name, dependency }
                if name == "ironafs-events" && dependency == "ironafs-time")
        }],
    },
    Case {
        name: "a pure crate with no library target",
        plant: |fixture| {
            fixture.file("ironafs-config", "src/lib.rs", None);
            fixture.file("ironafs-config", "src/main.rs", Some("fn main() {}\n"));
        },
        expect: &[
            |found| matches!(found, Violation::NoLibrary(name) if name == "ironafs-config"),
            |found| {
                matches!(found, Violation::ShippedTarget { name, target }
                    if name == "ironafs-config" && target == "bin")
            },
        ],
    },
    Case {
        name: "a pure crate with a build script",
        plant: |fixture| fixture.file("ironafs-config", "build.rs", Some("fn main() {}\n")),
        expect: &[|found| {
            matches!(found, Violation::ShippedTarget { name, target }
                if name == "ironafs-config" && target == "custom-build")
        }],
    },
    Case {
        name: "a module named tests without #[cfg(test)]",
        plant: |fixture| {
            fixture.file(
                PLANTED_CRATE,
                "src/lib.rs",
                Some("mod codec;\nmod tests;\n"),
            );
        },
        expect: &[
            |found| is_source(found, "crates/ironafs-proto/src/tests.rs", "std::fs"),
            |found| {
                is_source(
                    found,
                    "crates/ironafs-proto/src/tests.rs",
                    "std::time::Instant",
                )
            },
        ],
    },
    Case {
        name: "a std::net type in a pure crate",
        plant: |fixture| {
            plant_source(
                fixture,
                "fn f() { let _ = std::net::UdpSocket::bind(a); }\n",
            );
        },
        expect: &[|found| is_planted(found, "std::net::UdpSocket::bind")],
    },
    Case {
        name: "std::fs imported by a pure crate",
        plant: |fixture| plant_source(fixture, "use std::fs;\n"),
        expect: &[|found| is_planted(found, "std::fs")],
    },
    Case {
        name: "std::fs through raw identifiers",
        plant: |fixture| plant_source(fixture, "fn f() { r#std::r#fs::read(p); }\n"),
        expect: &[|found| is_planted(found, "std::fs::read")],
    },
    Case {
        name: "std::thread in a grouped import",
        plant: |fixture| {
            plant_source(
                fixture,
                "use std::{io::Read, thread::{self, JoinHandle}};\n",
            );
        },
        expect: &[|found| is_planted(found, "std::thread")],
    },
    Case {
        name: "std::os in a pure crate",
        plant: |fixture| plant_source(fixture, "use std::os::unix::net::UnixStream;\n"),
        expect: &[|found| is_planted(found, "std::os")],
    },
    Case {
        name: "std::process in a pure crate",
        plant: |fixture| plant_source(fixture, "fn f() { std::process::exit(1); }\n"),
        expect: &[|found| is_planted(found, "std::process")],
    },
    Case {
        name: "std::env in a pure crate",
        plant: |fixture| plant_source(fixture, "fn f() { let _ = std::env::var(\"X\"); }\n"),
        expect: &[|found| is_planted(found, "std::env")],
    },
    Case {
        name: "a standard stream in a pure crate",
        plant: |fixture| plant_source(fixture, "use std::io::{Write, stdout};\n"),
        expect: &[|found| is_planted(found, "std::io::stdout")],
    },
    Case {
        name: "std::io imported as a module",
        plant: |fixture| plant_source(fixture, "use std::io;\n"),
        expect: &[|found| is_planted(found, "std::io")],
    },
    Case {
        name: "a printing macro in a pure crate",
        plant: |fixture| plant_source(fixture, "fn f() { eprintln!(\"x\"); }\n"),
        expect: &[|found| is_planted(found, "eprintln!")],
    },
    Case {
        name: "the monotonic clock read directly",
        plant: |fixture| plant_source(fixture, "fn now() { std::time::Instant::now(); }\n"),
        expect: &[|found| is_planted(found, "std::time::Instant::now")],
    },
    Case {
        name: "the wall clock imported beside Duration",
        plant: |fixture| plant_source(fixture, "use std::time::{Duration, SystemTime};\n"),
        expect: &[|found| is_planted(found, "std::time::SystemTime")],
    },
    Case {
        name: "a glob of std::time",
        plant: |fixture| plant_source(fixture, "use std::time::*;\n"),
        expect: &[|found| is_planted(found, "std::time::*")],
    },
    Case {
        name: "a wait with a timeout",
        plant: |fixture| {
            plant_source(
                fixture,
                "fn f(r: &R, d: D) { let _ = r.recv_timeout(d); }\n",
            );
        },
        expect: &[|found| is_planted(found, "recv_timeout")],
    },
    Case {
        name: "a hash map seeded by the operating system",
        plant: |fixture| {
            plant_source(
                fixture,
                "use std::collections::HashMap;\nfn f() { let _ = HashMap::<u8, Vec<u8>>::new(); }\n",
            );
        },
        expect: &[|found| is_planted(found, "HashMap::new")],
    },
    Case {
        name: "the standard library's random hasher state",
        plant: |fixture| plant_source(fixture, "use std::hash::RandomState;\n"),
        expect: &[|found| is_planted(found, "RandomState")],
    },
    Case {
        name: "a file-system method on a path",
        plant: |fixture| plant_source(fixture, "fn f(p: &P) -> bool { p.exists() }\n"),
        expect: &[|found| is_planted(found, "exists")],
    },
    Case {
        name: "the standard library renamed",
        plant: |fixture| plant_source(fixture, "use std as host;\n"),
        expect: &[|found| is_planted(found, "std")],
    },
    Case {
        name: "the standard library passed to a macro",
        plant: |fixture| {
            plant_source(
                fixture,
                "macro_rules! v { ($($t:tt)*) => { std $($t)* }; }\nfn f() { v!(::fs::read(p)); }\n",
            );
        },
        expect: &[|found| is_planted(found, "std")],
    },
    Case {
        name: "a macro path fragment",
        plant: |fixture| {
            plant_source(
                fixture,
                "macro_rules! v { ($i:ident) => { use $i::fs as mfs; }; }\nv!(core);\n",
            );
        },
        expect: &[|found| is_planted(found, "$i::")],
    },
    Case {
        name: "a module loaded by a path attribute",
        plant: |fixture| {
            plant_source(
                fixture,
                "#[path = \"../../ironafs-net/src/lib.rs\"]\nmod net;\n",
            );
        },
        expect: &[|found| is_planted(found, "#[path]")],
    },
    Case {
        name: "source pulled in by include!",
        plant: |fixture| {
            plant_source(fixture, "include!(\"../../ironafs-net/src/lib.rs\");\n");
        },
        expect: &[|found| is_planted(found, "include!")],
    },
    Case {
        name: "a socket in ironafs-client",
        plant: |fixture| {
            fixture.file(
                "ironafs-client",
                "src/lib.rs",
                Some("use std::net::UdpSocket;\n"),
            );
        },
        expect: &[|found| is_source(found, CLIENT, "std::net::UdpSocket")],
    },
    Case {
        name: "a sleep in ironafs-client",
        plant: |fixture| {
            fixture.file(
                "ironafs-client",
                "src/lib.rs",
                Some("fn f(d: D) { std::thread::sleep(d); }\n"),
            );
        },
        expect: &[|found| is_source(found, CLIENT, "sleep")],
    },
];

/// Whether the violations are what the case planted.
fn as_planted(case: &Case, found: &[Violation]) -> bool {
    case.expect.iter().all(|expect| found.iter().any(expect))
        && found
            .iter()
            .all(|violation| case.expect.iter().any(|expect| expect(violation)))
}

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
    if !found.is_empty() || scanned != clean_scanned_files() {
        return Err(failure("the clean fixture", &found));
    }
    report(
        output,
        format!(
            "passed: the clean fixture ({} source files read)",
            scanned.len()
        ),
    )?;

    for (index, case) in CASES.iter().enumerate() {
        let mut planted = clean();
        (case.plant)(&mut planted);
        let (found, _) = check_fixture(&planted, &dir.join(format!("case-{index}")), case.name)?;
        match found.first() {
            Some(first) if as_planted(case, &found) => {
                report(output, format!("caught: {}: {first}", case.name))?;
            }
            _ => return Err(failure(case.name, &found)),
        }
    }

    let empty = Workspace {
        root: dir.to_path_buf(),
        target_dir: dir.to_path_buf(),
        members: Vec::new(),
        packages: Vec::new(),
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

/// The files the scan must read in the clean fixture, sorted: the library
/// root of every pure crate and of `ironafs-client`, and the module beside
/// one, but no gated unit-test file.
fn clean_scanned_files() -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = clean()
        .names()
        .into_iter()
        .filter(|name| super::kind_of(name).is_some_and(super::pure_source::is_scanned))
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
