// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! `check-deps --self-test`: plants each forbidden edge and each forbidden
//! name in a fixture workspace and proves the check fails on it with what
//! was planted, and passes on the clean fixture.

use std::io::Write;
use std::path::{Path, PathBuf};

use super::{Rule, THIRD_PARTY, ThirdPartyLists, Violation, check, graph_violations};
use crate::Error;
use crate::fixture::{self, Fixture};
use crate::metadata::Workspace;

const CHECK: &str = "check-deps";

/// The two third-party lists the fixtures replace. The repository's lists
/// are all empty, so these admit what the clean fixture uses, to prove that
/// an entry admits what it names and nothing else.
const FIXTURE_THIRD_PARTY: ThirdPartyLists<'static> = &[
    ("ironafs-os", &["libc"]),
    ("ironafs-gss", &["gss-ffi", "libc"]),
];

/// A workspace the rules allow, with one crate of each kind, a development
/// dependency on a test-only crate, a foreign-function crate in
/// `ironafs-os` and, through a crate its list names, in `ironafs-gss`, a
/// pure crate whose source this check does not read, the
/// `ironafs-client` of `clean_sources`, `ironafs-disk` in its gate-only
/// phase with the fault feature off by default, and `xtask` asking for
/// that feature in its dependency's feature list.
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
        ("ironafs-os", &["libc"]),
        ("ironafs-net", &["ironafs-rx", "ironafs-os"]),
        ("ironafs-gss", &["ironafs-time", "gss-ffi"]),
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
        ("ironafs-disk", &[]),
    ] {
        fixture.add_crate(name, dependencies);
    }
    fixture.lints("ironafs-disk", DISK_LINTS_AND_FEATURES);
    fixture.dependency_line("xtask", XTASK_ASKS_FOR_FAULT);
    fixture.add_third_party("libc", &[]);
    fixture.add_third_party("gss-ffi", &["libc"]);
    fixture.lints("ironafs-os", OS_LINTS);
    fixture.manifest_tail(
        "ironafs-cache",
        "\n[dev-dependencies]\nironafs-sim = { path = \"../ironafs-sim\" }\n\
         ironafs-test-support = { path = \"../ironafs-test-support\" }\n",
    );
    clean_sources(&mut fixture);
    fixture
}

/// The clean fixture's source files: `ironafs-client` uses threads, a
/// method of its own named like a file method, allowed standard-library
/// paths, an import of its own item named like a built-in macro, a raw
/// identifier spelled like a keyword, and gated unit-test modules that name
/// forbidden paths. The pure crates are not read: `check-pure` builds them.
fn clean_sources(fixture: &mut Fixture) {
    fixture.file(
        "ironafs-client",
        "src/lib.rs",
        Some(
            "//! Uses std::net only in this comment.\n\
             mod codec;\n#[cfg(test)]\nmod tests;\n\
             use std::thread::{self, JoinHandle};\n\
             pub fn run(f: fn()) -> JoinHandle<()> { thread::spawn(f) }\n\
             pub struct Node;\nimpl Node { pub fn exists(&self) -> bool { true } }\n\
             pub fn present(node: &Node) -> bool { node.exists() }\n",
        ),
    );
    fixture.file(
        "ironafs-client",
        "src/codec.rs",
        Some(
            "use std::io::Read;\nuse std::collections::BTreeMap;\nuse core::time::Duration;\n\
             use crate::report::show;\nuse self::inner::asm;\nlet r#unsafe = 1;\n\
             #[cfg(test)]\nmod tests;\n\
             pub fn f(_: Duration) -> &'static str { \"std::fs\" }\n",
        ),
    );
    fixture.file(
        "ironafs-client",
        "src/tests.rs",
        Some("use std::fs;\nuse std::time::Instant;\n"),
    );
    fixture.file(
        "ironafs-client",
        "src/codec/tests.rs",
        Some("mod helper;\nuse std::net::UdpSocket;\n"),
    );
    fixture.file(
        "ironafs-client",
        "src/codec/tests/helper.rs",
        Some("fn f() { std::thread::sleep(d); println!(\"x\"); }\n"),
    );
    fixture.file(
        "ironafs-proto",
        "src/lib.rs",
        Some("extern crate std;\nuse std::net::UdpSocket;\n"),
    );
}

/// The gate-only `ironafs-disk`'s inherited lints and its features in the
/// fixture. A case adds a feature by appending its line to the manifest,
/// which follows this table.
const DISK_LINTS_AND_FEATURES: &str = "\n[lints]\nworkspace = true\n\n[features]\nfault = []\n";

/// `xtask`'s request for the fault feature, the one C4 allows.
const XTASK_ASKS_FOR_FAULT: &str =
    "ironafs-disk = { path = \"../ironafs-disk\", features = [\"fault\"] }";

/// A dependency line on `ironafs-disk` from another crate under
/// `crates/`, with the rest of its entry.
macro_rules! on_disk {
    ($rest:literal) => {
        concat!("ironafs-disk = { path = \"../ironafs-disk\"", $rest, " }")
    };
}

/// `ironafs-os`'s own lint table in the fixture: the fixture workspace's
/// one lint, with unsafe code denied instead of forbidden.
const OS_LINTS: &str = "\n[lints.rust]\nunsafe_code = \"deny\"\n";

/// The file a planted source case writes, in `ironafs-client`.
const PLANTED_CRATE: &str = "ironafs-client";
const PLANTED: &str = "crates/ironafs-client/src/planted.rs";

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

fn is_planted_unreadable(violation: &Violation) -> bool {
    matches!(violation, Violation::Unreadable { file, .. } if file == Path::new(PLANTED))
}

/// Whether the violation is `word` found in the raw text of the planted
/// file at `line`.
fn is_planted_text(violation: &Violation, line: usize, word: &str) -> bool {
    matches!(violation, Violation::NamedInText { file, line: at, word: found }
        if file == Path::new(PLANTED) && *at == line && *found == word)
}

fn is_gate_only_table(violation: &Violation, declared: &str) -> bool {
    matches!(violation, Violation::DependencyTable { name, line, .. }
        if name == "ironafs-disk" && line == &format!("declares the {declared}"))
}

fn is_linked(violation: &Violation, from: &str, to: &str) -> bool {
    is_edge(violation, from, to, Rule::RealAndSimulatedLinked)
}

fn is_namesake(violation: &Violation, name: &str) -> bool {
    matches!(violation, Violation::Namesake { name: found, .. } if found == name)
}

fn is_requested(violation: &Violation, from: &str, feature: &str) -> bool {
    matches!(violation, Violation::FaultRequested { from: found, feature: asked }
        if found == from && asked == feature)
}

fn is_forwarded(violation: &Violation, from: &str, feature: &str, enabled: &str) -> bool {
    matches!(violation, Violation::FaultForwarded { from: found, feature: own, enables }
        if found == from && own == feature && enables == enabled)
}

/// Writes a third-party package named `name`, version 1.0.0, into the
/// fixture's local registry copy under a directory of its own, so a case
/// can depend on a package that shares a workspace crate's name. `tail`
/// follows its `[package]` table.
fn vendor_same_name(fixture: &mut Fixture, name: &str, tail: &str) {
    // Any third-party crate makes the fixture serve its registry copy.
    fixture.add_third_party("plain", &[]);
    let dir = format!("../vendor/registry-{name}");
    fixture.root_file(
        &format!("{dir}/Cargo.toml"),
        &format!("[package]\nname = \"{name}\"\nversion = \"1.0.0\"\nedition = \"2024\"\n{tail}"),
    );
    fixture.root_file(&format!("{dir}/src/lib.rs"), "");
    fixture.root_file(
        &format!("{dir}/.cargo-checksum.json"),
        "{\"files\":{},\"package\":null}",
    );
}

/// The third-party lists the fixtures are checked with: the repository's,
/// with only the two of `FIXTURE_THIRD_PARTY` replaced, so a case on any
/// other crate is checked under the list production uses.
fn fixture_third_party() -> Vec<(&'static str, &'static [&'static str])> {
    THIRD_PARTY
        .iter()
        .map(|(name, allowed)| {
            FIXTURE_THIRD_PARTY
                .iter()
                .find(|(replaced, _)| replaced == name)
                .copied()
                .unwrap_or((*name, *allowed))
        })
        .collect()
}

fn is_third_party(violation: &Violation, from: &str, to: &str) -> bool {
    matches!(violation, Violation::ThirdParty { from: found_from, to: found_to }
        if found_from == from && found_to == to)
}

fn is_without_lints(violation: &Violation, name: &str) -> bool {
    matches!(violation, Violation::NoWorkspaceLints { name: found, manifest }
        if found == name && manifest == Path::new(&format!("crates/{name}/Cargo.toml")))
}

fn is_own_lints(violation: &Violation, name: &str) -> bool {
    matches!(violation, Violation::OwnLints { name: found, manifest, differences }
        if found == name
            && manifest == Path::new(&format!("crates/{name}/Cargo.toml"))
            && differences.len() == 1)
}

fn is_without_edition(violation: &Violation, name: &str) -> bool {
    matches!(violation, Violation::NoWorkspaceEdition { name: found, manifest }
        if found == name && manifest == Path::new(&format!("crates/{name}/Cargo.toml")))
}

fn is_edition(violation: &Violation, name: &str, edition: &str) -> bool {
    matches!(violation, Violation::Edition { name: found, edition: at }
        if found == name && at == edition)
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
        // ironafs-net brings ironafs-os with it: both are reported. The
        // simulator, which uses ironafs-client, then reaches ironafs-net.
        expect: &[
            |found| is_edge(found, "ironafs-client", "ironafs-net", Rule::CoreOnDriver),
            |found| is_edge(found, "ironafs-client", "ironafs-os", Rule::CoreOnDriver),
            |found| is_linked(found, "ironafs-sim", "ironafs-net"),
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
        name: "ironafs-client on a third-party crate",
        plant: |fixture| {
            fixture.add_third_party("socket2", &[]);
            fixture.depend("ironafs-client", "socket2");
        },
        expect: &[|found| is_third_party(found, "ironafs-client", "socket2")],
    },
    Case {
        name: "a driver on a third-party crate",
        plant: |fixture| {
            fixture.add_third_party("socket2", &[]);
            fixture.depend("ironafs-net", "socket2");
        },
        expect: &[|found| is_third_party(found, "ironafs-net", "socket2")],
    },
    Case {
        name: "a binary on a third-party crate",
        plant: |fixture| {
            fixture.add_third_party("clap", &[]);
            fixture.depend("ironafsd", "clap");
        },
        expect: &[|found| is_third_party(found, "ironafsd", "clap")],
    },
    Case {
        name: "ironafs-gss on a foreign-function crate its list does not name",
        plant: |fixture| {
            fixture.add_third_party("openssl-sys", &[]);
            fixture.depend("ironafs-gss", "openssl-sys");
        },
        expect: &[
            |found| is_third_party(found, "ironafs-gss", "openssl-sys"),
            |found| {
                matches!(found, Violation::CryptoOutsideHome { from, via, to }
                    if from == "ironafs-gss" && via.is_empty() && to == "openssl-sys")
            },
        ],
    },
    Case {
        name: "ironafs-os on a workspace crate outside its list",
        plant: |fixture| fixture.depend("ironafs-os", "ironafs-proto"),
        expect: &[|found| {
            is_edge(
                found,
                "ironafs-os",
                "ironafs-proto",
                Rule::DriverOutsideList,
            )
        }],
    },
    Case {
        name: "a cryptography crate outside ironafs-crypto and ironafs-os",
        plant: |fixture| {
            fixture.add_third_party("ring", &[]);
            fixture.depend("ironafs-client", "ring");
        },
        expect: &[
            |found| {
                matches!(found, Violation::CryptoOutsideHome { from, via, to }
                    if from == "ironafs-client" && via.is_empty() && to == "ring")
            },
            |found| is_third_party(found, "ironafs-client", "ring"),
        ],
    },
    Case {
        name: "a foreign-function crate reached through another third-party crate",
        plant: |fixture| {
            fixture.add_third_party("wrapper", &["libc"]);
            fixture.depend("ironafsd", "wrapper");
        },
        expect: &[
            |found| {
                matches!(found, Violation::CryptoOutsideHome { from, via, to }
                    if from == "ironafsd" && via == &["wrapper"] && to == "libc")
            },
            |found| is_third_party(found, "ironafsd", "wrapper"),
        ],
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
        name: "ironafs-client with no library target",
        plant: |fixture| {
            fixture.file("ironafs-client", "src/lib.rs", None);
            fixture.file("ironafs-client", "src/main.rs", Some("fn main() {}\n"));
        },
        expect: &[
            |found| matches!(found, Violation::NoLibrary(name) if name == "ironafs-client"),
            |found| {
                matches!(found, Violation::ShippedTarget { name, target }
                    if name == "ironafs-client" && target == "bin")
            },
        ],
    },
    Case {
        name: "ironafs-client with a build script",
        plant: |fixture| fixture.file("ironafs-client", "build.rs", Some("fn main() {}\n")),
        expect: &[|found| {
            matches!(found, Violation::ShippedTarget { name, target }
                if name == "ironafs-client" && target == "custom-build")
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
            |found| is_source(found, "crates/ironafs-client/src/tests.rs", "std::fs"),
            |found| {
                is_source(
                    found,
                    "crates/ironafs-client/src/tests.rs",
                    "std::time::Instant",
                )
            },
        ],
    },
    Case {
        name: "a socket",
        plant: |fixture| {
            plant_source(
                fixture,
                "fn f() { let _ = std::net::UdpSocket::bind(a); }\n",
            );
        },
        expect: &[|found| is_planted(found, "std::net::UdpSocket::bind")],
    },
    Case {
        name: "std::fs imported",
        plant: |fixture| plant_source(fixture, "use std::fs;\n"),
        expect: &[|found| is_planted(found, "std::fs")],
    },
    Case {
        name: "std::fs through raw identifiers",
        plant: |fixture| plant_source(fixture, "fn f() { r#std::r#fs::read(p); }\n"),
        expect: &[|found| is_planted(found, "std::fs::read")],
    },
    Case {
        name: "std::fs in a grouped import",
        plant: |fixture| plant_source(fixture, "use std::{io::Read, fs::{self, File}};\n"),
        expect: &[|found| is_planted(found, "std::fs")],
    },
    Case {
        name: "a path's file-system method",
        plant: |fixture| {
            plant_source(
                fixture,
                "fn f(p: &std::path::Path) -> bool { p.exists() }\n",
            );
        },
        expect: &[|found| is_planted(found, "std::path::Path")],
    },
    Case {
        name: "a path type imported for its methods",
        plant: |fixture| {
            plant_source(
                fixture,
                "use std::path::PathBuf;\nfn f(p: PathBuf) -> bool { p.is_dir() }\n",
            );
        },
        expect: &[|found| is_planted(found, "std::path::PathBuf")],
    },
    Case {
        name: "std::os",
        plant: |fixture| plant_source(fixture, "use std::os::unix::net::UnixStream;\n"),
        expect: &[|found| is_planted(found, "std::os")],
    },
    Case {
        name: "std::process",
        plant: |fixture| plant_source(fixture, "fn f() { std::process::exit(1); }\n"),
        expect: &[|found| is_planted(found, "std::process")],
    },
    Case {
        name: "std::env",
        plant: |fixture| plant_source(fixture, "fn f() { let _ = std::env::var(\"X\"); }\n"),
        expect: &[|found| is_planted(found, "std::env")],
    },
    Case {
        name: "a standard stream",
        plant: |fixture| plant_source(fixture, "use std::io::{Write, stdout};\n"),
        expect: &[|found| is_planted(found, "std::io::stdout")],
    },
    Case {
        name: "std::io imported as a module",
        plant: |fixture| plant_source(fixture, "use std::io;\n"),
        expect: &[|found| is_planted(found, "std::io")],
    },
    Case {
        name: "a printing macro",
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
        name: "a sleep",
        plant: |fixture| plant_source(fixture, "fn f(d: D) { std::thread::sleep(d); }\n"),
        expect: &[|found| is_planted(found, "sleep")],
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
        name: "a standard hash map",
        plant: |fixture| {
            plant_source(
                fixture,
                "use std::collections::HashMap;\nfn f() { let _ = HashMap::<u8, Vec<u8>>::new(); }\n",
            );
        },
        expect: &[|found| is_planted(found, "HashMap")],
    },
    Case {
        name: "a hash map with a fixed hasher",
        plant: |fixture| {
            plant_source(
                fixture,
                "fn f() { let _: HashMap<u8, u8, Fixed> = Default::default(); }\n",
            );
        },
        expect: &[|found| is_planted(found, "HashMap")],
    },
    Case {
        name: "a hash map behind a type alias",
        plant: |fixture| {
            plant_source(
                fixture,
                "type M = std::collections::HashMap<u8, u8>;\nfn f() -> M { M::new() }\n",
            );
        },
        expect: &[|found| is_planted(found, "HashMap")],
    },
    Case {
        name: "a hash set in a derived default",
        plant: |fixture| {
            plant_source(
                fixture,
                "#[derive(Default)]\npub struct S { seen: HashSet<u8> }\n",
            );
        },
        expect: &[|found| is_planted(found, "HashSet")],
    },
    Case {
        name: "the standard library's random hasher state",
        plant: |fixture| plant_source(fixture, "use std::hash::RandomState;\n"),
        expect: &[|found| is_planted(found, "RandomState")],
    },
    Case {
        name: "a backtrace",
        plant: |fixture| {
            plant_source(
                fixture,
                "fn f() { let _ = std::backtrace::Backtrace::capture(); }\n",
            );
        },
        expect: &[|found| is_planted(found, "std::backtrace")],
    },
    Case {
        name: "include! renamed by an import",
        plant: |fixture| plant_source(fixture, "use std::include as pull;\npull!(\"x.rs\");\n"),
        expect: &[
            |found| is_planted_text(found, 1, "include"),
            |found| is_planted(found, "std::include"),
            |found| is_planted(found, "include"),
        ],
    },
    Case {
        name: "println! renamed by an import",
        plant: |fixture| {
            plant_source(
                fixture,
                "use std::println as say;\nfn f() { say!(\"x\"); }\n",
            );
        },
        expect: &[
            |found| is_planted(found, "std::println"),
            |found| is_planted(found, "println"),
        ],
    },
    Case {
        name: "include! imported from a renamed core",
        plant: |fixture| {
            plant_source(
                fixture,
                "extern crate core as c;\nuse c::include as pull;\npull!(\"x.rs\");\n",
            );
        },
        expect: &[
            |found| is_planted_text(found, 2, "include"),
            |found| is_planted(found, "c::include"),
            |found| is_planted(found, "include"),
        ],
    },
    Case {
        name: "assembly renamed by an import",
        plant: |fixture| {
            plant_source(
                fixture,
                "use core::arch::global_asm as go;\ngo!(\"nop\");\n",
            );
        },
        expect: &[
            |found| is_planted(found, "core::arch::global_asm"),
            |found| is_planted(found, "global_asm"),
        ],
    },
    Case {
        name: "unsafe foreign functions",
        plant: |fixture| {
            plant_source(
                fixture,
                "unsafe extern \"C\" { safe fn getpid() -> i32; }\npub fn pid() -> i32 { getpid() }\n",
            );
        },
        expect: &[
            |found| is_planted(found, "unsafe"),
            |found| is_planted(found, "extern"),
        ],
    },
    Case {
        name: "assembly without an unsafe block",
        plant: |fixture| plant_source(fixture, "core::arch::global_asm!(\"nop\");\n"),
        expect: &[|found| is_planted(found, "global_asm!")],
    },
    Case {
        name: "a foreign-function definition",
        plant: |fixture| plant_source(fixture, "pub extern \"C\" fn callback() {}\n"),
        expect: &[|found| is_planted(found, "extern")],
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
        name: "include! called through a macro fragment",
        plant: |fixture| {
            plant_source(
                fixture,
                "macro_rules! call { ($m:ident) => { $m!(\"../../ironafs-net/src/lib.rs\"); }; }\n\
                 call!(include);\n",
            );
        },
        expect: &[
            |found| is_planted_text(found, 2, "include"),
            |found| is_planted(found, "$m!"),
            |found| is_planted(found, "include"),
        ],
    },
    Case {
        name: "include! joined to its bang by a macro",
        plant: |fixture| {
            plant_source(
                fixture,
                "macro_rules! pull { ($bang:tt $name:ident) => { $name $bang(\"../../ironafs-net/src/lib.rs\"); }; }\n\
                 pull!(! include);\n",
            );
        },
        expect: &[
            |found| is_planted_text(found, 2, "include"),
            |found| is_planted(found, "include"),
        ],
    },
    Case {
        name: "global_asm! joined to its bang by a macro",
        plant: |fixture| {
            plant_source(
                fixture,
                "macro_rules! pull { ($bang:tt $name:ident) => { $name $bang(\"nop\"); }; }\n\
                 pull!(! global_asm);\n",
            );
        },
        expect: &[|found| is_planted(found, "global_asm")],
    },
    Case {
        name: "println! joined to its bang by a macro",
        plant: |fixture| {
            plant_source(
                fixture,
                "macro_rules! pull { ($bang:tt $name:ident) => { $name $bang(\"x\"); }; }\n\
                 fn f() { pull!(! println); }\n",
            );
        },
        expect: &[|found| is_planted(found, "println")],
    },
    Case {
        name: "a path attribute whose brackets are a macro fragment",
        plant: |fixture| {
            plant_source(
                fixture,
                "macro_rules! load { ($group:tt) => { # $group mod net; }; }\n\
                 load!([path = \"../../ironafs-net/src/lib.rs\"]);\n",
            );
        },
        expect: &[|found| is_planted(found, "#")],
    },
    Case {
        name: "a path attribute built from a macro fragment",
        plant: |fixture| {
            plant_source(
                fixture,
                "macro_rules! load { ($a:meta) => { #[$a] mod net; }; }\n\
                 load!(path = \"../../ironafs-net/src/lib.rs\");\n",
            );
        },
        expect: &[|found| is_planted(found, "#[$")],
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
        expect: &[
            |found| is_planted_text(found, 1, "include"),
            |found| is_planted(found, "include!"),
        ],
    },
    Case {
        name: "include! hidden behind a string with a suffix",
        plant: |fixture| {
            plant_source(
                fixture,
                "macro_rules! drop_all { ($($t:tt)*) => {}; }\n\
                 drop_all!(\"\"r\"\\\" \");\n\
                 include!(\"../../ironafs-net/src/lib.rs\"); // \"\n",
            );
        },
        expect: &[
            |found| is_planted_text(found, 3, "include"),
            is_planted_unreadable,
        ],
    },
    Case {
        name: "include! after a lifetime spelled like a raw string's prefix",
        plant: |fixture| {
            plant_source(
                fixture,
                "macro_rules! drop_all { ($($t:tt)*) => {}; }\n\
                 drop_all!('r\"\\\" \");\n\
                 include!(\"../../ironafs-net/src/lib.rs\"); // \"\n",
            );
        },
        expect: &[
            |found| is_planted_text(found, 3, "include"),
            |found| is_planted(found, "include!"),
        ],
    },
    Case {
        name: "a shebang line the compiler skips unread",
        plant: |fixture| plant_source(fixture, "#!/usr/bin/env x \"\nfn f() {} // \"\n"),
        expect: &[is_planted_unreadable],
    },
    Case {
        name: "a frontmatter fence whose content the compiler skips unread",
        plant: |fixture| plant_source(fixture, "\n---\nx = \"\n---\nfn f() {} // \"\n"),
        expect: &[is_planted_unreadable],
    },
    Case {
        name: "a file read at compile time by include_str!",
        plant: |fixture| {
            plant_source(
                fixture,
                "const KEY: &str = include_str!(\"../../../deploy/secret.key\");\n",
            );
        },
        expect: &[
            |found| is_planted(found, "include_str!"),
            |found| is_planted_text(found, 1, "include_str"),
        ],
    },
    Case {
        name: "include_bytes! joined to its bang by a macro",
        plant: |fixture| {
            plant_source(
                fixture,
                "macro_rules! pull { ($bang:tt $name:ident) => { $name $bang(\"/etc/passwd\"); }; }\n\
                 pull!(! include_bytes);\n",
            );
        },
        expect: &[
            |found| is_planted(found, "include_bytes"),
            |found| is_planted_text(found, 2, "include_bytes"),
        ],
    },
    Case {
        name: "a file-reading macro named in a file the scan does not compile",
        plant: |fixture| {
            fixture.file(
                PLANTED_CRATE,
                "tests/it/main.rs",
                Some("// include_bytes!(\"x\")\n"),
            );
        },
        expect: &[|found| {
            matches!(found, Violation::NamedInText { file, line: 1, word: "include_bytes" }
                if file == Path::new("crates/ironafs-client/tests/it/main.rs"))
        }],
    },
    Case {
        name: "include! hidden by a C string prefix an older edition does not have",
        plant: |fixture| {
            fixture.edition(PLANTED_CRATE, "edition = \"2018\"");
            plant_source(
                fixture,
                "macro_rules! drop_all { ($($t:tt)*) => {}; }\n\
                 drop_all!(cr\"\\\" \");\n\
                 include!(\"../../ironafs-net/src/lib.rs\"); // \"\n",
            );
        },
        expect: &[
            |found| is_without_edition(found, PLANTED_CRATE),
            |found| is_edition(found, PLANTED_CRATE, "2018"),
            |found| is_planted_text(found, 3, "include"),
        ],
    },
    Case {
        name: "a target in an older edition than its package",
        plant: |fixture| {
            fixture.manifest_tail("ironafs-xdr", "\n[lib]\nedition = \"2021\"\n");
        },
        expect: &[
            |found| is_without_edition(found, "ironafs-xdr"),
            |found| is_edition(found, "ironafs-xdr", "2021"),
        ],
    },
    Case {
        name: "a package in the workspace's edition named outright",
        plant: |fixture| fixture.edition("ironafs-cache", "edition = \"2024\""),
        expect: &[|found| is_without_edition(found, "ironafs-cache")],
    },
    Case {
        name: "a pure crate with an empty lints table",
        plant: |fixture| fixture.lints("ironafs-auth", "\n[lints]\n"),
        expect: &[|found| is_without_lints(found, "ironafs-auth")],
    },
    Case {
        name: "a binary that overrides the workspace lints",
        plant: |fixture| {
            fixture.lints("ironafsd", "\n[lints.rust]\nunsafe_code = \"allow\"\n");
        },
        expect: &[|found| is_without_lints(found, "ironafsd")],
    },
    Case {
        name: "a lints table only inside a multi-line string",
        plant: |fixture| {
            fixture.lints(
                "ironafsd",
                "\n[package.metadata.notes]\ntext = \"\"\"\n[lints]\nworkspace = true\n[notes]\n\"\"\"\n",
            );
        },
        expect: &[
            |found| is_without_edition(found, "ironafsd"),
            |found| is_without_lints(found, "ironafsd"),
        ],
    },
    Case {
        name: "ironafs-os allowing unsafe code in its own table",
        plant: |fixture| fixture.lints("ironafs-os", "\n[lints.rust]\nunsafe_code = \"allow\"\n"),
        expect: &[|found| is_own_lints(found, "ironafs-os")],
    },
    Case {
        name: "ironafs-os lowering a lint the workspace sets",
        plant: |fixture| {
            fixture.lints(
                "ironafs-os",
                &format!("{OS_LINTS}unsafe_op_in_unsafe_fn = \"allow\"\n"),
            );
        },
        expect: &[|found| is_own_lints(found, "ironafs-os")],
    },
    Case {
        name: "ironafs-os inheriting the workspace table, which forbids its sites",
        plant: |fixture| fixture.lints("ironafs-os", "\n[lints]\nworkspace = true\n"),
        expect: &[|found| is_own_lints(found, "ironafs-os")],
    },
    Case {
        name: "a target-scoped lints table",
        plant: |fixture| {
            fixture.lints(
                "ironafs-net",
                "\n[lints]\nworkspace = true\n\n[target.'cfg(unix)'.lints.rust]\nunsafe_code = \"allow\"\n",
            );
        },
        expect: &[|found| is_without_lints(found, "ironafs-net")],
    },
    Case {
        name: "the gate-only ironafs-disk with a normal dependency",
        plant: |fixture| fixture.depend("ironafs-disk", "ironafs-time"),
        // Its driver list is empty in this phase: both rules report it.
        expect: &[
            |found| is_gate_only_table(found, "normal ironafs-time"),
            |found| {
                is_edge(
                    found,
                    "ironafs-disk",
                    "ironafs-time",
                    Rule::DriverOutsideList,
                )
            },
        ],
    },
    Case {
        name: "the gate-only ironafs-disk with a build dependency",
        plant: |fixture| {
            fixture.manifest_tail(
                "ironafs-disk",
                "\n[build-dependencies]\nironafs-time = { path = \"../ironafs-time\" }\n",
            );
        },
        expect: &[
            |found| is_gate_only_table(found, "build ironafs-time"),
            |found| {
                is_edge(
                    found,
                    "ironafs-disk",
                    "ironafs-time",
                    Rule::DriverOutsideList,
                )
            },
        ],
    },
    Case {
        name: "the gate-only ironafs-disk with a development dependency",
        plant: |fixture| {
            fixture.manifest_tail(
                "ironafs-disk",
                "\n[dev-dependencies]\nironafs-test-support = { path = \"../ironafs-test-support\" }\n",
            );
        },
        expect: &[|found| is_gate_only_table(found, "dev ironafs-test-support")],
    },
    Case {
        name: "the gate-only ironafs-disk with a target-specific development dependency",
        plant: |fixture| {
            fixture.manifest_tail(
                "ironafs-disk",
                "\n[target.'cfg(unix)'.dev-dependencies]\nironafs-time = { path = \"../ironafs-time\" }\n",
            );
        },
        expect: &[|found| is_gate_only_table(found, "dev ironafs-time")],
    },
    Case {
        name: "the gate-only ironafs-disk with a third-party development dependency",
        plant: |fixture| {
            fixture.add_third_party("plain", &[]);
            fixture.manifest_tail("ironafs-disk", "\n[dev-dependencies]\nplain = \"1\"\n");
        },
        expect: &[|found| is_gate_only_table(found, "dev plain")],
    },
    Case {
        name: "the gate-only ironafs-disk with a third-party dependency",
        plant: |fixture| {
            fixture.add_third_party("plain", &[]);
            fixture.depend("ironafs-disk", "plain");
        },
        expect: &[
            |found| is_gate_only_table(found, "normal plain"),
            |found| is_third_party(found, "ironafs-disk", "plain"),
        ],
    },
    Case {
        name: "ironafs-disk with its own lints table",
        plant: |fixture| {
            fixture.lints(
                "ironafs-disk",
                "\n[lints.rust]\nunsafe_code = \"allow\"\n\n[features]\nfault = []\n",
            );
        },
        expect: &[|found| is_without_lints(found, "ironafs-disk")],
    },
    Case {
        name: "ironafs-test-support with a featureless development dependency on ironafs-disk",
        plant: |fixture| {
            fixture.manifest_tail(
                "ironafs-test-support",
                concat!("\n[dev-dependencies]\n", on_disk!(""), "\n"),
            );
        },
        expect: &[|found| {
            is_edge(
                found,
                "ironafs-test-support",
                "ironafs-disk",
                Rule::SupportOnDisk,
            )
        }],
    },
    Case {
        name: "ironafs-test-support depending on ironafs-disk",
        plant: |fixture| fixture.depend("ironafs-test-support", "ironafs-disk"),
        expect: &[|found| {
            is_edge(
                found,
                "ironafs-test-support",
                "ironafs-disk",
                Rule::SupportOnDisk,
            )
        }],
    },
    Case {
        name: "ironafs-net with a development dependency on ironafs-disk",
        plant: |fixture| {
            fixture.manifest_tail(
                "ironafs-net",
                concat!("\n[dev-dependencies]\n", on_disk!(""), "\n"),
            );
        },
        expect: &[|found| is_linked(found, "ironafs-net", "ironafs-disk")],
    },
    Case {
        name: "ironafs-disk with a development dependency on ironafs-net",
        plant: |fixture| {
            fixture.manifest_tail(
                "ironafs-disk",
                "\n[dev-dependencies]\nironafs-net = { path = \"../ironafs-net\" }\n",
            );
        },
        // The gate-only phase refuses every dependency as well.
        expect: &[
            |found| is_linked(found, "ironafs-disk", "ironafs-net"),
            |found| is_gate_only_table(found, "dev ironafs-net"),
        ],
    },
    Case {
        name: "ironafs-disk with a development dependency on ironafs-sim",
        plant: |fixture| {
            fixture.manifest_tail(
                "ironafs-disk",
                "\n[dev-dependencies]\nironafs-sim = { path = \"../ironafs-sim\" }\n",
            );
        },
        expect: &[
            |found| is_linked(found, "ironafs-disk", "ironafs-sim"),
            |found| is_gate_only_table(found, "dev ironafs-sim"),
        ],
    },
    Case {
        name: "ironafs-sim on ironafs-disk",
        plant: |fixture| fixture.depend("ironafs-sim", "ironafs-disk"),
        expect: &[|found| is_linked(found, "ironafs-sim", "ironafs-disk")],
    },
    Case {
        name: "ironafs-sim with a development dependency on ironafs-net",
        plant: |fixture| {
            fixture.manifest_tail(
                "ironafs-sim",
                "\n[dev-dependencies]\nironafs-net = { path = \"../ironafs-net\" }\n",
            );
        },
        expect: &[|found| is_linked(found, "ironafs-sim", "ironafs-net")],
    },
    Case {
        name: "ironafs-net with a development dependency on ironafs-sim",
        plant: |fixture| {
            fixture.manifest_tail(
                "ironafs-net",
                "\n[dev-dependencies]\nironafs-sim = { path = \"../ironafs-sim\" }\n",
            );
        },
        expect: &[|found| is_linked(found, "ironafs-net", "ironafs-sim")],
    },
    Case {
        name: "ironafs-sim reaching ironafs-net through a binary",
        plant: |fixture| fixture.depend("ironafs-sim", "ironafsd"),
        expect: &[|found| {
            matches!(found, Violation::Edge { from, via, to, rule: Rule::RealAndSimulatedLinked }
                if from == "ironafs-sim" && to == "ironafs-net" && via == &["ironafsd"])
        }],
    },
    Case {
        name: "a pure crate on ironafs-disk",
        plant: |fixture| fixture.depend("ironafs-auth", "ironafs-disk"),
        expect: &[|found| is_edge(found, "ironafs-auth", "ironafs-disk", Rule::PureOnNonPure)],
    },
    Case {
        name: "a layer crate on ironafs-disk",
        plant: |fixture| fixture.depend("ironafs-fuse", "ironafs-disk"),
        expect: &[|found| {
            is_edge(
                found,
                "ironafs-fuse",
                "ironafs-disk",
                Rule::LayerBypassesCore,
            )
        }],
    },
    Case {
        name: "ironafs-client on ironafs-disk",
        plant: |fixture| fixture.depend("ironafs-client", "ironafs-disk"),
        // The simulator, which uses ironafs-client, then reaches it too.
        expect: &[
            |found| is_edge(found, "ironafs-client", "ironafs-disk", Rule::CoreOnDriver),
            |found| is_linked(found, "ironafs-sim", "ironafs-disk"),
        ],
    },
    Case {
        name: "ironafs-disk without the fault feature",
        plant: |fixture| {
            fixture.remove_dependency_line("xtask", XTASK_ASKS_FOR_FAULT);
            fixture.lints(
                "ironafs-disk",
                "\n[lints]\nworkspace = true\n\n[features]\nother = []\n",
            );
        },
        expect: &[
            |found| matches!(found, Violation::FaultUndeclared(name) if name == "ironafs-disk"),
        ],
    },
    Case {
        name: "the fault feature on by default",
        plant: |fixture| fixture.manifest_tail("ironafs-disk", "default = [\"fault\"]\n"),
        expect: &[|found| {
            matches!(found, Violation::FaultByDefault { name, via }
                if name == "ironafs-disk" && via.is_empty())
        }],
    },
    Case {
        name: "the fault feature on by default through another feature",
        plant: |fixture| {
            fixture.manifest_tail(
                "ironafs-disk",
                "default = [\"extra\"]\nextra = [\"fault\"]\n",
            );
        },
        expect: &[|found| {
            matches!(found, Violation::FaultByDefault { name, via }
                if name == "ironafs-disk" && via == &["extra"])
        }],
    },
    Case {
        name: "a test-only crate the graph rules allow asking for the fault feature",
        plant: |fixture| {
            fixture.add_crate("ironafs-bans-canary", &[]);
            fixture.dependency_line("ironafs-bans-canary", on_disk!(", features = [\"fault\"]"));
        },
        expect: &[|found| is_requested(found, "ironafs-bans-canary", "fault")],
    },
    Case {
        name: "a shipped crate asking for the fault feature",
        plant: |fixture| fixture.dependency_line("ironafsd", on_disk!(", features = [\"fault\"]")),
        expect: &[|found| is_requested(found, "ironafsd", "fault")],
    },
    Case {
        name: "a shipped crate asking for the fault feature in a development dependency",
        plant: |fixture| {
            fixture.manifest_tail(
                "ironafs-cache",
                concat!(on_disk!(", features = [\"fault\"]"), "\n"),
            );
        },
        expect: &[|found| is_requested(found, "ironafs-cache", "fault")],
    },
    Case {
        name: "a shipped crate asking for a feature that turns the fault feature on",
        plant: |fixture| {
            fixture.manifest_tail("ironafs-disk", "extra = [\"fault\"]\n");
            fixture.dependency_line("ironafsd", on_disk!(", features = [\"extra\"]"));
        },
        expect: &[|found| is_requested(found, "ironafsd", "extra")],
    },
    Case {
        name: "a shipped crate asking for the fault feature under another name",
        plant: |fixture| {
            fixture.dependency_line(
                "ironafsd",
                "disk = { package = \"ironafs-disk\", path = \"../ironafs-disk\", features = [\"fault\"] }",
            );
        },
        expect: &[|found| is_requested(found, "ironafsd", "fault")],
    },
    Case {
        name: "xtask forwarding the fault feature from its own features",
        plant: |fixture| {
            fixture.manifest_tail(
                "xtask",
                "\n[features]\nexercise = [\"ironafs-disk/fault\"]\n",
            );
        },
        expect: &[|found| is_forwarded(found, "xtask", "exercise", "ironafs-disk/fault")],
    },
    Case {
        name: "xtask turning on an optional dependency that asks for the fault feature",
        plant: |fixture| {
            fixture.remove_dependency_line("xtask", XTASK_ASKS_FOR_FAULT);
            fixture.dependency_line(
                "xtask",
                on_disk!(", optional = true, features = [\"fault\"]"),
            );
            fixture.manifest_tail("xtask", "\n[features]\nexercise = [\"dep:ironafs-disk\"]\n");
        },
        expect: &[|found| is_forwarded(found, "xtask", "exercise", "dep:ironafs-disk")],
    },
    Case {
        name: "a shipped crate forwarding the fault feature from its own features",
        plant: |fixture| {
            fixture.dependency_line("ironafsd", on_disk!(""));
            fixture.manifest_tail(
                "ironafsd",
                "\n[features]\nfaults = [\"ironafs-disk/fault\"]\n",
            );
        },
        expect: &[|found| is_forwarded(found, "ironafsd", "faults", "ironafs-disk/fault")],
    },
    Case {
        name: "a test-only crate forwarding the fault feature weakly under another name",
        plant: |fixture| {
            fixture.add_crate("ironafs-bans-canary", &[]);
            fixture.dependency_line(
                "ironafs-bans-canary",
                "d = { package = \"ironafs-disk\", path = \"../ironafs-disk\", optional = true }",
            );
            // Turning the dependency on asks for nothing; the weak request does.
            fixture.manifest_tail(
                "ironafs-bans-canary",
                "\n[features]\nx = [\"dep:d\"]\ny = [\"d?/fault\"]\n",
            );
        },
        expect: &[|found| is_forwarded(found, "ironafs-bans-canary", "y", "d?/fault")],
    },
    Case {
        name: "a layer crate reaching ironafs-disk through a crate its list allows",
        plant: |fixture| fixture.depend("ironafs-ipc", "ironafs-disk"),
        expect: &[
            |found| is_edge(found, "ironafs-ipc", "ironafs-disk", Rule::PureOnNonPure),
            |found| {
                matches!(found, Violation::Edge { from, via, to, rule: Rule::LayerBypassesCore }
                    if from == "ironafs-fuse" && to == "ironafs-disk" && via == &["ironafs-ipc"])
            },
        ],
    },
    Case {
        name: "a shipped crate asking for the fault feature on an optional dependency",
        plant: |fixture| {
            fixture.dependency_line(
                "ironafsd",
                on_disk!(", optional = true, features = [\"fault\"]"),
            );
            fixture.manifest_tail("ironafsd", "\n[features]\ndisk = [\"dep:ironafs-disk\"]\n");
        },
        expect: &[
            |found| is_requested(found, "ironafsd", "fault"),
            |found| is_forwarded(found, "ironafsd", "disk", "dep:ironafs-disk"),
        ],
    },
    Case {
        name: "xtask turning on an asking optional dependency through a strong feature path",
        plant: |fixture| {
            fixture.remove_dependency_line("xtask", XTASK_ASKS_FOR_FAULT);
            fixture.manifest_tail("ironafs-disk", "harmless = []\n");
            fixture.dependency_line(
                "xtask",
                "d = { package = \"ironafs-disk\", path = \"../ironafs-disk\", optional = true, features = [\"fault\"] }",
            );
            fixture.manifest_tail("xtask", "\n[features]\nx = [\"d/harmless\"]\n");
        },
        // Cargo also gives the optional dependency its implicit feature.
        expect: &[
            |found| is_forwarded(found, "xtask", "x", "d/harmless"),
            |found| is_forwarded(found, "xtask", "d", "dep:d"),
        ],
    },
    Case {
        name: "a third-party crate asking for the fault feature through a patch",
        plant: |fixture| {
            fixture.add_third_party("helper", &[]);
            fixture.dependency_line(
                "helper",
                "ironafs-disk = { version = \"0.0.0\", features = [\"fault\"] }",
            );
            fixture.depend("xtask", "helper");
            fixture.workspace_tail(
                "\n[patch.crates-io]\nironafs-disk = { path = \"crates/ironafs-disk\" }\n",
            );
        },
        expect: &[|found| is_requested(found, "helper", "fault")],
    },
    Case {
        name: "a shipped crate asking for the fault feature of a custom-named rlib",
        plant: |fixture| {
            fixture.manifest_tail(
                "ironafs-disk",
                "\n[lib]\nname = \"gate\"\ncrate-type = [\"rlib\"]\n",
            );
            fixture.dependency_line("ironafsd", on_disk!(", features = [\"fault\"]"));
        },
        expect: &[|found| is_requested(found, "ironafsd", "fault")],
    },
    Case {
        name: "a shipped crate asking for the fault feature under its own name as a rename",
        plant: |fixture| {
            fixture.manifest_tail("ironafs-disk", "\n[lib]\nname = \"gate\"\n");
            fixture.dependency_line(
                "ironafsd",
                "ironafs-disk = { package = \"ironafs-disk\", path = \"../ironafs-disk\", features = [\"fault\"] }",
            );
        },
        expect: &[|found| is_requested(found, "ironafsd", "fault")],
    },
    Case {
        name: "a crate asking a registry namesake of ironafs-disk for a feature the home's \
               turns fault on with",
        plant: |fixture| {
            // The workspace disk, renamed `d`, has a library named `d` too.
            fixture.manifest_tail(
                "ironafs-disk",
                "extra = [\"fault\"]\n\n[lib]\nname = \"d\"\n",
            );
            vendor_same_name(fixture, "ironafs-disk", "\n[features]\nextra = []\n");
            fixture.add_crate("ironafs-bans-canary", &[]);
            fixture.dependency_line(
                "ironafs-bans-canary",
                "d = { package = \"ironafs-disk\", path = \"../ironafs-disk\" }",
            );
            fixture.dependency_line(
                "ironafs-bans-canary",
                "ironafs-disk = { version = \"1\", features = [\"extra\"] }",
            );
        },
        // The namesake is refused, and the declaration is read as one of the
        // workspace crate, whose `extra` turns `fault` on.
        expect: &[
            |found| is_namesake(found, "ironafs-disk"),
            |found| is_requested(found, "ironafs-bans-canary", "extra"),
        ],
    },
    Case {
        name: "ironafs-test-support on a registry namesake of ironafs-disk",
        plant: |fixture| {
            vendor_same_name(fixture, "ironafs-disk", "");
            fixture.dependency_line("ironafs-test-support", "ironafs-disk = \"1\"");
        },
        expect: &[
            |found| is_namesake(found, "ironafs-disk"),
            |found| {
                is_edge(
                    found,
                    "ironafs-test-support",
                    "ironafs-disk",
                    Rule::SupportOnDisk,
                )
            },
        ],
    },
    Case {
        name: "ironafs-net with a development dependency on a registry namesake of ironafs-sim",
        plant: |fixture| {
            vendor_same_name(fixture, "ironafs-sim", "");
            fixture.manifest_tail("ironafs-net", "\n[dev-dependencies]\nironafs-sim = \"1\"\n");
        },
        expect: &[
            |found| is_namesake(found, "ironafs-sim"),
            |found| is_linked(found, "ironafs-net", "ironafs-sim"),
        ],
    },
    Case {
        name: "xtask on a registry namesake of ironafs-sim",
        plant: |fixture| {
            vendor_same_name(fixture, "ironafs-sim", "");
            fixture.dependency_line(
                "xtask",
                "sim = { package = \"ironafs-sim\", version = \"1\" }",
            );
        },
        expect: &[|found| is_namesake(found, "ironafs-sim")],
    },
    Case {
        name: "xtask on a local namesake of ironafs-net outside the workspace",
        plant: |fixture| {
            fixture.add_outside_crate("ironafs-net", &[]);
            // Cargo refuses two local packages of one name and version.
            fixture.version("ironafs-net", "9.0.0");
            fixture.dependency_line(
                "xtask",
                "net = { package = \"ironafs-net\", path = \"../../../outside/ironafs-net\" }",
            );
        },
        expect: &[
            |found| matches!(found, Violation::NotAMember { name, .. } if name == "ironafs-net"),
            |found| is_namesake(found, "ironafs-net"),
        ],
    },
    Case {
        name: "a third-party crate asking for the fault feature behind a feature no one turns on",
        plant: |fixture| {
            fixture.add_third_party("helper", &[]);
            fixture.dependency_line(
                "helper",
                "ironafs-disk = { version = \"0.0.0\", optional = true, features = [\"fault\"] }",
            );
            fixture.manifest_tail("helper", "\n[features]\nx = [\"dep:ironafs-disk\"]\n");
            fixture.depend("xtask", "helper");
            fixture.workspace_tail(
                "\n[patch.crates-io]\nironafs-disk = { path = \"crates/ironafs-disk\" }\n",
            );
        },
        expect: &[
            |found| is_requested(found, "helper", "fault"),
            |found| is_forwarded(found, "helper", "x", "dep:ironafs-disk"),
        ],
    },
    Case {
        name: "ironafs-sim reaching ironafs-net through a third-party crate's inactive dependency",
        plant: |fixture| {
            fixture.add_third_party("relay", &[]);
            fixture.dependency_line(
                "relay",
                "ironafs-net = { version = \"0.0.0\", optional = true }",
            );
            fixture.depend("ironafs-sim", "relay");
            fixture.workspace_tail(
                "\n[patch.crates-io]\nironafs-net = { path = \"crates/ironafs-net\" }\n",
            );
        },
        expect: &[|found| {
            matches!(found, Violation::Edge { from, via, to, rule: Rule::RealAndSimulatedLinked }
                if from == "ironafs-sim" && to == "ironafs-net" && via == &["relay"])
        }],
    },
    Case {
        name: "a binary-only crate asking for the fault feature in a development dependency",
        plant: |fixture| {
            fixture.add_crate("ironafs-cli", &[]);
            fixture.file("ironafs-cli", "src/lib.rs", None);
            fixture.file("ironafs-cli", "src/main.rs", Some("fn main() {}\n"));
            fixture.manifest_tail(
                "ironafs-cli",
                concat!(
                    "\n[dev-dependencies]\n",
                    on_disk!(", features = [\"fault\"]"),
                    "\n"
                ),
            );
        },
        expect: &[|found| is_requested(found, "ironafs-cli", "fault")],
    },
    Case {
        name: "a shipped crate declaring ironafs-disk twice, the development one asking for fault",
        plant: |fixture| {
            fixture.dependency_line("ironafsd", on_disk!(""));
            fixture.manifest_tail(
                "ironafsd",
                concat!(
                    "\n[dev-dependencies]\n",
                    on_disk!(", features = [\"fault\"]"),
                    "\n"
                ),
            );
        },
        expect: &[|found| is_requested(found, "ironafsd", "fault")],
    },
];

/// A plant the rules allow: its fixture must still pass.
struct Control {
    name: &'static str,
    plant: fn(&mut Fixture),
}

const CONTROLS: &[Control] = &[
    Control {
        name: "ironafs-test-support reaching ironafs-disk through a binary it depends on",
        plant: |fixture| {
            fixture.depend("ironafsd", "ironafs-disk");
            fixture.depend("ironafs-test-support", "ironafsd");
        },
    },
    Control {
        name: "ironafs-sim with a development dependency on itself",
        plant: |fixture| {
            fixture.manifest_tail(
                "ironafs-sim",
                "\n[dev-dependencies]\nironafs-sim = { path = \".\" }\n",
            );
        },
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

    for (index, control) in CONTROLS.iter().enumerate() {
        let mut planted = clean();
        (control.plant)(&mut planted);
        let (found, _) = check_fixture(
            &planted,
            &dir.join(format!("control-{index}")),
            control.name,
        )?;
        if !found.is_empty() {
            return Err(failure(control.name, &found));
        }
        report(output, format!("passed: {}", control.name))?;
    }

    run_own_fixtures(dir, output)?;

    // A third-party crate declares ironafs-net, behind a feature no one turns
    // on, in a workspace with no member of that name.
    let mut no_member = Fixture::default();
    no_member.add_crate("xtask", &["helper"]);
    no_member.add_third_party("helper", &[]);
    no_member.dependency_line(
        "helper",
        "ironafs-net = { version = \"1\", optional = true }",
    );
    let workspace = no_member.load(
        &dir.join("no-member"),
        CHECK,
        "a declaration of ironafs-net with no such member",
    )?;
    let (found, _) = check(&workspace, &fixture_third_party(), &[])?;
    let as_planted = matches!(found.as_slice(), [Violation::NoMemberForName { from, name }]
        if from == "helper" && name == "ironafs-net");
    if !as_planted {
        return Err(failure(
            "a declaration of ironafs-net with no such member",
            &found,
        ));
    }
    report(
        output,
        "caught: a declaration of ironafs-net with no such member".to_owned(),
    )?;

    // None of the three crates, and no declaration of any: nothing to refuse.
    let mut absent = Fixture::default();
    absent.add_crate("ironafs-time", &[]);
    absent.add_crate("xtask", &["ironafs-time"]);
    let workspace = absent.load(&dir.join("absent"), CHECK, "the three crates absent")?;
    let (found, _) = check(&workspace, &fixture_third_party(), &[])?;
    if !found.is_empty() {
        return Err(failure("the three crates absent", &found));
    }
    report(output, "passed: the three crates absent".to_owned())?;

    let empty = Workspace {
        root: dir.to_path_buf(),
        target_dir: dir.to_path_buf(),
        members: Vec::new(),
        packages: Vec::new(),
    };
    let found = graph_violations(&empty, &fixture_third_party());
    if found != [Violation::EmptyWorkspace] {
        return Err(failure("an empty workspace", &found));
    }
    report(output, "caught: an empty workspace".to_owned())?;
    report(
        output,
        format!(
            "check-deps --self-test: the clean fixture and {} controls passed and {} planted \
             cases failed as planted",
            CONTROLS.len() + 1,
            CASES.len() + 5
        ),
    )
}

/// The cases that need a fixture of their own rather than the clean one.
///
/// # Errors
///
/// Returns the first case that did not end as planted.
fn run_own_fixtures(dir: &Path, output: &mut impl Write) -> Result<(), Error> {
    let report = |output: &mut dyn Write, line: String| {
        writeln!(output, "{line}").map_err(Error::io("report check-deps self-test"))
    };
    // The clean fixture's ironafs-os depends on libc, which its list allows;
    // under the repository's rule that ironafs-os has no dependency table at
    // all, that is caught.
    let workspace = clean().load(&dir.join("tables"), CHECK, "dependency tables")?;
    let (found, _) = check(&workspace, &fixture_third_party(), &["ironafs-os"])?;
    let as_planted = matches!(found.as_slice(), [Violation::DependencyTable { name, .. }] if name == "ironafs-os");
    if !as_planted {
        return Err(failure("ironafs-os with a dependency table", &found));
    }
    report(
        output,
        "caught: ironafs-os with a dependency table".to_owned(),
    )?;

    // A development dependency under an escaped quoted key, which no text
    // search for "dependencies" finds, and which the resolved graph drops.
    let mut escaped = Fixture::default();
    escaped.add_crate("ironafs-os", &[]);
    escaped.add_crate("ironafs-time", &[]);
    escaped.lints("ironafs-os", OS_LINTS);
    escaped.manifest_tail(
        "ironafs-os",
        "\n[\"dev-depend\\u0065ncies\"]\nironafs-time = { path = \"../ironafs-time\" }\n",
    );
    let workspace = escaped.load(&dir.join("escaped"), CHECK, "an escaped dependency key")?;
    let found = super::dependency_table_violations(&workspace, &["ironafs-os"]);
    let as_planted = matches!(found.as_slice(), [Violation::DependencyTable { name, line, .. }]
        if name == "ironafs-os" && line == "declares the dev ironafs-time");
    if !as_planted {
        return Err(failure(
            "ironafs-os with a dev-dependency under an escaped key",
            &found,
        ));
    }
    report(
        output,
        "caught: ironafs-os with a dev-dependency under an escaped key".to_owned(),
    )?;

    // The fault feature's home moved out of the workspace, still reached by
    // path and turning the feature on by default, is refused, so its rules
    // cannot be escaped that way.
    let mut outside = Fixture::default();
    outside.add_crate("xtask", &[]);
    outside.add_outside_crate("ironafs-disk", &[]);
    outside.manifest_tail(
        "ironafs-disk",
        "\n[features]\ndefault = [\"fault\"]\nfault = []\n",
    );
    outside.dependency_line(
        "xtask",
        "ironafs-disk = { path = \"../../../outside/ironafs-disk\", features = [\"fault\"] }",
    );
    let workspace = outside.load(
        &dir.join("outside"),
        CHECK,
        "ironafs-disk outside the workspace",
    )?;
    let (found, _) = check(&workspace, &fixture_third_party(), &[])?;
    // It is a local crate outside the workspace, a namesake, and what xtask
    // declares has no member of that name: each rule reports it.
    let as_planted = matches!(found.as_slice(), [
        Violation::NotAMember { name, .. },
        Violation::Namesake { name: namesake, .. },
        Violation::NoMemberForName { from, name: declared },
    ] if name == "ironafs-disk" && namesake == "ironafs-disk" && from == "xtask"
        && declared == "ironafs-disk");
    if !as_planted {
        return Err(failure("ironafs-disk outside the workspace", &found));
    }
    report(
        output,
        "caught: ironafs-disk outside the workspace".to_owned(),
    )?;

    Ok(())
}

/// The files the scan must read in the clean fixture, sorted: the library
/// root of `ironafs-client` and the module beside it, but no gated
/// unit-test file and no file of a pure crate.
fn clean_scanned_files() -> Vec<PathBuf> {
    vec![
        PathBuf::from("crates/ironafs-client/src/codec.rs"),
        PathBuf::from("crates/ironafs-client/src/lib.rs"),
    ]
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
    check(&workspace, &fixture_third_party(), &[])
}
