// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Proves each seam rule fires on planted sources (seams and scheduling
//! specification, 5.3, point 2, "Planted self-tests"; section 9's rows for
//! `seams::deadline_built_from_a_fresh_sample` and
//! `seams::pure_crate_calls_no_seam`), and records the two limits as
//! planted passes.
//!
//! The plants are held in memory and go through the same `check` as the
//! workspace. The file walk that feeds the workspace run is shown working
//! by that run itself, which fails if it reads nothing.

use std::io::Write;
use std::path::{Path, PathBuf};

use super::{CHECK, Sampling, SourceFile, Violation, check, renamed_dependencies, source_file};
use crate::Error;
use crate::metadata::Member;
use crate::rust_source::TokenKind;

/// The sampling function the plants use.
const SAMPLING: &[Sampling] = &[Sampling {
    krate: "planted-core",
    path: "planted_core::time::deadline_after",
}];

/// A sampling function as the rules want it, and one caller per allowed
/// `from`.
const CONTROL: &str = "
#[expect(clippy::disallowed_methods, reason = \"the one sampling function\")]
pub fn deadline_after(
    clock: &dyn Clock,
    kind: ClockKind,
    from: &Stamp,
    d: Duration,
) -> Result<Deadline, DeadlineError> {
    let now = clock.sample().map_err(DeadlineError::Clock)?;
    match kind {
        ClockKind::Boot => Deadline::boot_after(&now, from, d),
        _ => Deadline::awake_after(&now, from, d),
    }
}

pub fn for_request(clock: &dyn Clock, request: &Request, d: Duration) -> Result<Deadline, DeadlineError> {
    deadline_after(clock, ClockKind::Boot, &request.arrival, d)
}

pub fn for_reply(clock: &dyn Clock, reply: &Reply, d: Duration) -> Option<Deadline> {
    deadline_after(clock, ClockKind::Boot, reply.first_sent.as_ref()?, d).ok()
}

pub fn for_reply_or(clock: &dyn Clock, reply: &Reply, d: Duration) -> Result<Deadline, DeadlineError> {
    deadline_after(clock, ClockKind::Boot, reply.first_sent.as_ref().ok_or(DeadlineError::FromAfterNow)?, d)
}

#[cfg(test)]
mod planted_tests {
    fn anywhere(now: &Stamp) { let _ = Deadline::boot_after(now, now, Duration::ZERO); }
}
";

/// The pure crates' control: definitions and root re-exports.
const PURE_ROOT: &str =
    "mod seam;\npub use seam::{\n    Clock, ClockError, ClockKind,\n    Rng,\n};\n";
const PURE_SEAM: &str = "pub trait Clock: Send + Sync {\n    fn boot(&self) -> u64;\n    fn sample(&self) -> u64;\n}\npub trait Rng: Send + Sync { fn fill(&self, bytes: &mut [u8]); }\n";
const PURE_RX: &str = "pub enum Failure { Clock(u8), Rng }\nimpl Failure { pub fn failed() -> Self { Self::Clock(0) } }\npub trait Rpc: Send + Sync { fn call(&self, request: &[u8]); }\npub fn close(endpoint: &mut Endpoint) { endpoint.close(); }\n";

/// Planted sources as the token reader gave them: each a file, or the
/// reason it could not be read, kept until the case that owns it fails.
type Fixture = Vec<Result<SourceFile, Violation>>;

fn core(source: &str) -> Fixture {
    core_at("src/time.rs", source)
}

fn core_at(path: &str, source: &str) -> Fixture {
    vec![source_file("planted-core", path, false, false, source)]
}

/// The library of check-bans' canary: its constructor lines, taken from
/// the ban list it is written from, in the crate named `krate`.
fn canary_in(krate: &str) -> Fixture {
    let lines: String = crate::bans::BANS
        .iter()
        .filter(|ban| ban.path.starts_with("ironafs_time::Deadline::"))
        .flat_map(|ban| [ban.planted, "\n"])
        .collect();
    vec![source_file(krate, "src/lib.rs", false, true, &lines)]
}

fn pure(krate: &str, path: &str, source: &str) -> Fixture {
    vec![source_file(krate, path, true, path == "src/lib.rs", source)]
}

/// The production `Rng`'s shape (5.2, point 3): its import and its impl.
const PURE_CRYPTO: &str = "use ironafs_time::{Rng, RngError};\npub struct CryptoRng;\nimpl Rng for CryptoRng {\n    fn fill(&self, bytes: &mut [u8]) -> Result<(), RngError> { Ok(()) }\n}\n";

fn pure_control() -> Fixture {
    let mut files = pure("ironafs-time", "src/lib.rs", PURE_ROOT);
    files.extend(pure("ironafs-time", "src/seam.rs", PURE_SEAM));
    files.extend(pure("ironafs-rx", "src/rpc.rs", PURE_RX));
    files.extend(pure("ironafs-crypto", "src/random.rs", PURE_CRYPTO));
    files
}

fn with_pure(extra: Fixture) -> Fixture {
    let mut files = pure_control();
    files.extend(extra);
    files
}

/// The control with `from` replaced at the request caller.
fn with_from(from: &str) -> String {
    CONTROL.replace("&request.arrival", from)
}

/// The control's boot constructor call replaced by `body`.
fn with_body(body: &str) -> String {
    CONTROL.replace(
        "ClockKind::Boot => Deadline::boot_after(&now, from, d),",
        body,
    )
}

/// One planted case: its sources, the one kind of violation it must give,
/// and an identifier on the planted line, where that kind has a line: the
/// violation must be on a line holding it. So a plant cannot pass because
/// some other rule fired somewhere else.
struct Case {
    name: &'static str,
    files: fn() -> Fixture,
    kind: fn(&Violation) -> bool,
    at: Option<&'static str>,
}

/// The time crate's error enum, as a pure crate's plants see it.
fn time_error_enum() -> Fixture {
    pure(
        "ironafs-time",
        "src/wait.rs",
        "pub enum DeadlineError { Clock(u8), Overflow }\n",
    )
}

const CASES: &[Case] = &[
    Case {
        // Only the canary is exempt, by its exact package name.
        name: "the canary's constructor lines in another crate",
        files: || canary_in("planted-core"),
        kind: |found| matches!(found, Violation::Outside { .. }),
        at: Some("boot_after"),
    },
    Case {
        name: "the canary's constructor lines in a crate named like it",
        files: || canary_in("ironafs-bans-canary2"),
        kind: |found| matches!(found, Violation::Outside { .. }),
        at: Some("awake_after"),
    },
    Case {
        name: "the canary's constructor lines in a crate spelt like it",
        files: || canary_in("ironafs_bans_canary"),
        kind: |found| matches!(found, Violation::Outside { .. }),
        at: Some("boot_after"),
    },
    Case {
        name: "a `now` kept from an earlier sample",
        files: || {
            core(&with_body(
                "ClockKind::Boot => Deadline::boot_after(kept, from, d),",
            ))
        },
        kind: |found| matches!(found, Violation::NowNotFresh { .. }),
        at: Some("kept"),
    },
    Case {
        name: "a `now` passed in, with no fresh sample",
        files: || {
            core(&CONTROL.replace(
                "let now = clock.sample().map_err(DeadlineError::Clock)?;",
                "let now = clock.kept();",
            ))
        },
        kind: |found| matches!(found, Violation::NoFreshSample { .. }),
        at: None,
    },
    Case {
        name: "a fresh sample taken from a parameter that is not the clock",
        files: || {
            core(
                &CONTROL
                    .replace(
                        "    kind: ClockKind,\n",
                        "    kind: ClockKind,\n    fake: &Fake,\n",
                    )
                    .replace("let now = clock.sample()", "let now = fake.sample()"),
            )
        },
        kind: |found| matches!(found, Violation::NoFreshSample { .. }),
        at: None,
    },
    Case {
        name: "the fresh sample's binding reassigned",
        files: || {
            core(&CONTROL.replace(
                "let now = clock.sample().map_err(DeadlineError::Clock)?;",
                "let now = clock.sample().map_err(DeadlineError::Clock)?;\n    now = earlier;",
            ))
        },
        kind: |found| matches!(found, Violation::Reassigned { .. }),
        at: Some("earlier"),
    },
    Case {
        name: "a parameter that shares the fresh sample's name",
        files: || {
            core(&CONTROL.replace(
                "    kind: ClockKind,\n",
                "    kind: ClockKind,\n    now: Stamp,\n",
            ))
        },
        kind: |found| matches!(found, Violation::Reassigned { .. }),
        at: None,
    },
    Case {
        name: "a constructor before the fresh sample is bound",
        files: || {
            core(&CONTROL.replace(
                "    let now = clock.sample().map_err(DeadlineError::Clock)?;\n    match kind {\n        ClockKind::Boot => Deadline::boot_after(&now, from, d),",
                "    let early = Deadline::boot_after(&now, from, d);\n    let now = clock.sample().map_err(DeadlineError::Clock)?;\n    match kind {\n        ClockKind::Boot => early,",
            ))
        },
        kind: |found| matches!(found, Violation::NowNotFresh { .. }),
        at: Some("early"),
    },
    Case {
        name: "a pattern binding that shadows the fresh sample",
        files: || {
            core(&with_body(
                "ClockKind::Boot => { if let Some(now) = kept { return Deadline::boot_after(&now, from, d); } Deadline::boot_after(&now, from, d) }",
            ))
        },
        kind: |found| matches!(found, Violation::Reassigned { .. }),
        at: Some("kept"),
    },
    Case {
        name: "a constructor whose `from` is another parameter",
        files: || {
            core(&with_body(
                "ClockKind::Boot => Deadline::boot_after(&now, forged, d),",
            ))
        },
        kind: |found| matches!(found, Violation::NotFromParameter { .. }),
        at: Some("forged"),
    },
    Case {
        name: "a constructor called outside the sampling function",
        files: || {
            core(&format!(
                "{CONTROL}\npub fn other(now: &Stamp, d: Duration) -> Result<Deadline, DeadlineError> {{ Deadline::awake_after(now, now, d) }}\n"
            ))
        },
        kind: |found| matches!(found, Violation::Outside { .. }),
        at: Some("other"),
    },
    Case {
        name: "a second function in the same crate with its own allowance",
        files: || {
            core(&format!(
                "{CONTROL}\n#[expect(clippy::disallowed_methods, reason = \"a second\")] pub fn second(clock: &dyn Clock, from: &Stamp, d: Duration) -> Result<Deadline, DeadlineError> {{ let now = clock.sample().map_err(DeadlineError::Clock)?; Deadline::boot_after(&now, from, d) }}\n"
            ))
        },
        kind: |found| {
            matches!(
                found,
                Violation::Allowance {
                    on: "another function",
                    ..
                }
            )
        },
        at: Some("second"),
    },
    Case {
        name: "a module-level allowance",
        files: || {
            core(&format!(
                "#![expect(clippy::disallowed_methods, reason = \"the module\")] use crate::module_level;\n{CONTROL}"
            ))
        },
        kind: |found| {
            matches!(
                found,
                Violation::Allowance {
                    on: "a module or the crate",
                    ..
                }
            )
        },
        at: Some("module_level"),
    },
    Case {
        name: "an allowance on a module item",
        files: || {
            core(&format!(
                "{CONTROL}\n#[expect(clippy::disallowed_methods, reason = \"the module\")] mod time;\n"
            ))
        },
        kind: |found| matches!(found, Violation::Allowance { on: "a module", .. }),
        at: Some("time"),
    },
    Case {
        name: "the constructor carried out by a closure",
        files: || {
            core(&with_body(
                "ClockKind::Boot => { let build = |x: Duration| Deadline::boot_after(&now, from, x); build(d) }",
            ))
        },
        kind: |found| matches!(found, Violation::Hides { what, .. } if what.starts_with("a closure")),
        at: Some("build"),
    },
    Case {
        name: "the constructor carried out by a function pointer",
        files: || {
            core(&with_body(
                "ClockKind::Boot => { let build = Deadline::boot_after; build(&now, from, d) }",
            ))
        },
        kind: |found| matches!(found, Violation::AsValue { .. }),
        at: Some("build"),
    },
    Case {
        name: "the constructor inside a macro call",
        files: || {
            core(&with_body(
                "ClockKind::Boot => identity!(Deadline::boot_after(&now, from, d)),",
            ))
        },
        kind: |found| matches!(found, Violation::InMacro { .. }),
        at: Some("identity"),
    },
    Case {
        name: "a nested function in the sampling function",
        files: || {
            core(&with_body(
                "ClockKind::Boot => { fn inner() {} Deadline::boot_after(&now, from, d) }",
            ))
        },
        kind: |found| {
            matches!(
                found,
                Violation::Hides {
                    what: "a nested function",
                    ..
                }
            )
        },
        at: Some("inner"),
    },
    Case {
        name: "a helper in `ironafs-time` that builds a deadline",
        files: || {
            pure(
                "ironafs-time",
                "src/wait.rs",
                "impl Deadline {\n    pub fn boot_after(now: &Stamp, from: &Stamp, d: Duration) -> Result<Self, DeadlineError> { build() }\n}\npub fn deadline(now: &Stamp, from: &Stamp, d: Duration) -> Result<Deadline, DeadlineError> { Deadline::boot_after(now, from, d) }\n",
            )
        },
        kind: |found| matches!(found, Violation::Outside { .. }),
        at: Some("deadline"),
    },
    Case {
        name: "a `#[path]` module, out of the walk's sight",
        files: || {
            core(&format!(
                "{CONTROL}\n#[path = \"../elsewhere.rs\"] mod elsewhere;\n"
            ))
        },
        kind: |found| matches!(found, Violation::PathAttribute { .. }),
        at: Some("elsewhere"),
    },
    Case {
        name: "a `path` inside `cfg_attr`, out of the walk's sight",
        files: || {
            core(&format!(
                "{CONTROL}\n#[cfg_attr(all(), path = \"tests.rs\")] mod hidden;\n"
            ))
        },
        kind: |found| matches!(found, Violation::PathAttribute { .. }),
        at: Some("hidden"),
    },
    Case {
        name: "an `include!` of source from elsewhere",
        files: || core(&format!("{CONTROL}\ninclude!(\"../generated.rs\");\n")),
        kind: |found| matches!(found, Violation::IncludeMacro { .. }),
        at: Some("include"),
    },
    Case {
        name: "a `from` that is a fresh sample",
        files: || core(&with_from("&clock.sample()?")),
        kind: |found| matches!(found, Violation::BadFrom { .. }),
        at: Some("sample"),
    },
    Case {
        name: "a `from` that is a forged `Stamp` literal with a high count",
        files: || {
            core(&with_from(
                "&Stamp { boot, awake, wall, doubt: Doubt { boot: u64::MAX, awake: u64::MAX }, episodes: 0 }",
            ))
        },
        kind: |found| matches!(found, Violation::BadFrom { .. }),
        at: Some("episodes"),
    },
    Case {
        name: "a `from` that is a local",
        files: || core(&with_from("&arrived")),
        kind: |found| matches!(found, Violation::BadFrom { .. }),
        at: Some("arrived"),
    },
    Case {
        name: "the sampling function renamed on import",
        files: || {
            core(&format!(
                "{CONTROL}\nuse crate::time::deadline_after as make;\n"
            ))
        },
        kind: |found| matches!(found, Violation::Renamed { .. }),
        at: Some("make"),
    },
    Case {
        name: "the sampling function passed as a value",
        files: || {
            core(&format!(
                "{CONTROL}\npub fn table() -> usize {{ let f = deadline_after; 0 }}\n"
            ))
        },
        kind: |found| matches!(found, Violation::AsValue { .. }),
        at: Some("table"),
    },
    Case {
        name: "the sampling function defined twice",
        files: || {
            let mut files = core(CONTROL);
            files.extend(core(CONTROL));
            files
        },
        kind: |found| matches!(found, Violation::Duplicate { .. }),
        at: None,
    },
    Case {
        name: "the listed sampling function missing from its crate",
        files: || core("pub fn other() {}\n"),
        kind: |found| matches!(found, Violation::Missing { .. }),
        at: None,
    },
    Case {
        name: "a sampling function with no `from` parameter",
        files: || core(&CONTROL.replace("    from: &Stamp,\n", "    start: &Stamp,\n")),
        kind: |found| matches!(found, Violation::NoFrom { .. }),
        at: Some("deadline_after"),
    },
    Case {
        name: "the clock parameter rebound before the sample",
        files: || {
            core(&CONTROL.replace(
                "    let now = clock.sample()",
                "    let clock = &Rebound;\n    let now = clock.sample()",
            ))
        },
        kind: |found| matches!(found, Violation::NoFreshSample { .. }),
        at: Some("Rebound"),
    },
    Case {
        name: "the sampling function defined in another module: the definition",
        files: || core_at("src/other.rs", CONTROL),
        kind: |found| matches!(found, Violation::Missing { .. }),
        at: None,
    },
    Case {
        name: "the sampling function defined in another module: its constructors",
        files: || core_at("src/other.rs", CONTROL),
        kind: |found| matches!(found, Violation::Outside { .. }),
        at: Some("boot_after"),
    },
    Case {
        name: "the sampling function defined in a file ending like its module, outside the library",
        files: || core_at("examples/app/src/time.rs", CONTROL),
        kind: |found| matches!(found, Violation::Missing { .. }),
        at: None,
    },
    Case {
        name: "the sampling function defined in a nested module",
        files: || core(&format!("mod nested {{\n{CONTROL}\n}}\n")),
        kind: |found| matches!(found, Violation::Outside { .. }),
        at: Some("boot_after"),
    },
    Case {
        name: "`include` renamed by a `use`",
        files: || {
            core(&format!(
                "{CONTROL}\nuse core::include as inc;\ninc!(\"../generated.rs\");\n"
            ))
        },
        kind: |found| matches!(found, Violation::IncludeMacro { .. }),
        at: Some("inc"),
    },
    Case {
        name: "an inner `#![cfg(test)]` in a module hides nothing else in its file",
        files: || {
            core(&format!(
                "mod gated {{ #![cfg(test)] }}\n{CONTROL}\npub fn other(now: &Stamp, d: Duration) -> Result<Deadline, DeadlineError> {{ Deadline::awake_after(now, now, d) }}\n"
            ))
        },
        kind: |found| matches!(found, Violation::Outside { .. }),
        at: Some("other"),
    },
    Case {
        name: "a `#[cfg(test)]` field hides nothing after it",
        files: || {
            with_pure(pure(
                "ironafs-rx",
                "src/endpoint.rs",
                "pub struct Endpoint {\n    #[cfg(test)]\n    planted: u8,\n    clock: &'static dyn ironafs_time::Clock,\n}\n",
            ))
        },
        kind: |found| matches!(found, Violation::SeamInPure { name: "Clock", .. }),
        at: Some("clock"),
    },
    Case {
        name: "a pure crate takes a seam as `&dyn`",
        files: || {
            with_pure(pure(
                "ironafs-rx",
                "src/endpoint.rs",
                "pub fn tick(clock: &dyn ironafs_time::Clock) -> u64 { 0 }\n",
            ))
        },
        kind: |found| matches!(found, Violation::SeamInPure { name: "Clock", .. }),
        at: Some("tick"),
    },
    Case {
        name: "a pure crate bounds a generic by a seam",
        files: || {
            with_pure(pure(
                "ironafs-cache",
                "src/plan.rs",
                "pub fn plan<D: Disk>(disk: &D) {}\n",
            ))
        },
        kind: |found| matches!(found, Violation::SeamInPure { name: "Disk", .. }),
        at: Some("plan"),
    },
    Case {
        name: "a pure crate implements a seam",
        files: || {
            with_pure(pure(
                "ironafs-cache",
                "src/fake.rs",
                "impl ironafs_time::Rng for Fake { fn fill(&self, bytes: &mut [u8]) {} }\n",
            ))
        },
        kind: |found| matches!(found, Violation::SeamInPure { name: "Rng", .. }),
        at: Some("Fake"),
    },
    Case {
        name: "a pure crate renames a seam on import",
        files: || {
            with_pure(pure(
                "ironafs-rx",
                "src/lib.rs",
                "pub use ironafs_time::Clock as Tick;\n",
            ))
        },
        kind: |found| matches!(found, Violation::SeamInPure { name: "Clock", .. }),
        at: Some("Tick"),
    },
    Case {
        name: "a seam named through a module path",
        files: || {
            with_pure(pure(
                "ironafs-cache",
                "src/plan.rs",
                "pub fn plan(disk: &crate::Disk) {}\n",
            ))
        },
        kind: |found| matches!(found, Violation::SeamInPure { name: "Disk", .. }),
        at: Some("plan"),
    },
    Case {
        name: "a raw identifier naming a seam",
        files: || {
            with_pure(pure(
                "ironafs-rx",
                "src/endpoint.rs",
                "pub fn tick(rpc: &dyn r#Rpc) {}\n",
            ))
        },
        kind: |found| matches!(found, Violation::SeamInPure { name: "Rpc", .. }),
        at: Some("tick"),
    },
    Case {
        name: "a seam renamed in its home crate's root re-export",
        files: || {
            let mut files = pure(
                "ironafs-time",
                "src/lib.rs",
                "pub use seam::Clock as Tick;\n",
            );
            files.extend(pure_control());
            files
        },
        kind: |found| matches!(found, Violation::SeamInPure { name: "Clock", .. }),
        at: Some("Tick"),
    },
    Case {
        name: "the production `Rng`'s crate holds the seam",
        files: || {
            with_pure(pure(
                "ironafs-crypto",
                "src/hold.rs",
                "use ironafs_time::Rng;\npub fn hold(rng: &dyn Rng) {}\n",
            ))
        },
        kind: |found| matches!(found, Violation::SeamInPure { name: "Rng", .. }),
        at: Some("hold"),
    },
    Case {
        name: "the production `Rng`'s crate calls the seam",
        files: || {
            with_pure(pure(
                "ironafs-crypto",
                "src/draw.rs",
                "pub fn draw(source: &CryptoRng) { let _ = Rng::fill(source, &mut []); }\n",
            ))
        },
        kind: |found| matches!(found, Violation::SeamInPure { name: "Rng", .. }),
        at: Some("draw"),
    },
    Case {
        name: "the production `Rng`'s crate implements another seam",
        files: || {
            with_pure(pure(
                "ironafs-crypto",
                "src/clock.rs",
                "use ironafs_time::Clock;\nimpl Clock for CryptoClock {}\n",
            ))
        },
        kind: |found| matches!(found, Violation::SeamInPure { name: "Clock", .. }),
        at: Some("CryptoClock"),
    },
    Case {
        name: "the production `Rng`'s crate renames the seam on import",
        files: || {
            with_pure(pure(
                "ironafs-crypto",
                "src/source.rs",
                "use ironafs_time::Rng as Source;\n",
            ))
        },
        kind: |found| matches!(found, Violation::SeamInPure { name: "Rng", .. }),
        at: Some("Source"),
    },
    Case {
        name: "the production `Rng`'s crate gives another item the seam's name on import",
        files: || {
            with_pure(pure(
                "ironafs-crypto",
                "src/source.rs",
                "use ironafs_time::{Clock as Rng};\n",
            ))
        },
        kind: |found| matches!(found, Violation::SeamInPure { name: "Rng", .. }),
        at: Some("use"),
    },
    Case {
        name: "the production `Rng`'s crate gives its seam's home crate the seam's name",
        files: || {
            with_pure(pure(
                "ironafs-crypto",
                "src/source.rs",
                "use ironafs_time as Rng;\n",
            ))
        },
        kind: |found| matches!(found, Violation::SeamInPure { name: "Rng", .. }),
        at: Some("use"),
    },
    Case {
        name: "the production `Rng`'s crate re-exports the seam",
        files: || {
            with_pure(pure(
                "ironafs-crypto",
                "src/lib.rs",
                "pub use ironafs_time::Rng;\n",
            ))
        },
        kind: |found| matches!(found, Violation::SeamInPure { name: "Rng", .. }),
        at: Some("pub"),
    },
    Case {
        name: "another pure crate implements the production `Rng`'s seam",
        files: || {
            with_pure(pure(
                "ironafs-rx",
                "src/fake.rs",
                "use ironafs_time::Rng;\nimpl Rng for FakeRng {}\n",
            ))
        },
        kind: |found| matches!(found, Violation::SeamInPure { name: "Rng", .. }),
        at: Some("FakeRng"),
    },
    Case {
        name: "a seam re-exported outside its home crate's root",
        files: || {
            with_pure(pure(
                "ironafs-time",
                "src/instant.rs",
                "pub use crate::Clock;\n",
            ))
        },
        kind: |found| matches!(found, Violation::SeamInPure { name: "Clock", .. }),
        at: Some("use"),
    },
    Case {
        name: "a seam through an uppercase alias of its home crate: the path",
        files: || {
            with_pure(pure(
                "ironafs-rx",
                "src/endpoint.rs",
                "use ironafs_time as Time;\npub fn tick(clock: &dyn Time::Clock) -> u64 { 0 }\n",
            ))
        },
        kind: |found| matches!(found, Violation::SeamInPure { name: "Clock", .. }),
        at: Some("tick"),
    },
    Case {
        name: "a seam through an uppercase alias of its home crate: the import",
        files: || {
            with_pure(pure(
                "ironafs-rx",
                "src/endpoint.rs",
                "use ironafs_time as Time;\npub fn tick(clock: &dyn Time::Clock) -> u64 { 0 }\n",
            ))
        },
        kind: |found| matches!(found, Violation::HomeImport { .. }),
        at: Some("use"),
    },
    Case {
        name: "a seam through a glob re-export behind an uppercase module: the path",
        files: || {
            with_pure(pure(
                "ironafs-cache",
                "src/plan.rs",
                "mod Time { pub use ironafs_time::*; }\npub fn plan(rng: &dyn Time::Rng) {}\n",
            ))
        },
        kind: |found| matches!(found, Violation::SeamInPure { name: "Rng", .. }),
        at: Some("plan"),
    },
    Case {
        name: "a seam through a glob re-export behind an uppercase module: the glob",
        files: || {
            with_pure(pure(
                "ironafs-cache",
                "src/plan.rs",
                "mod Time { pub use ironafs_time::*; }\npub fn plan(rng: &dyn Time::Rng) {}\n",
            ))
        },
        kind: |found| matches!(found, Violation::HomeImport { .. }),
        at: Some("mod"),
    },
    Case {
        name: "a `::`-rooted seam path through an enum named like its home crate: the path",
        files: || {
            with_pure(pure(
                "ironafs-rx",
                "src/endpoint.rs",
                "pub enum ironafs_time { Clock }\npub fn tick(clock: &dyn ::ironafs_time::Clock) {}\n",
            ))
        },
        kind: |found| matches!(found, Violation::SeamInPure { name: "Clock", .. }),
        at: Some("tick"),
    },
    Case {
        name: "a `::`-rooted seam path through an enum named like its home crate: the enum",
        files: || {
            with_pure(pure(
                "ironafs-rx",
                "src/endpoint.rs",
                "pub enum ironafs_time { Clock }\npub fn tick(clock: &dyn ::ironafs_time::Clock) {}\n",
            ))
        },
        kind: |found| matches!(found, Violation::HomeShadow { .. }),
        at: Some("enum"),
    },
    Case {
        name: "a home crate's enum name used as a path without its import (a renamed dependency)",
        files: || {
            let mut files = with_pure(time_error_enum());
            files.extend(pure(
                "ironafs-rx",
                "src/endpoint.rs",
                "pub fn tick(clock: &dyn DeadlineError::Clock) {}\n",
            ));
            files
        },
        kind: |found| matches!(found, Violation::SeamInPure { name: "Clock", .. }),
        at: Some("tick"),
    },
    Case {
        name: "a seam through its home crate renamed like a home enum, inside the home crate",
        files: || {
            let mut files = with_pure(time_error_enum());
            files.extend(pure(
                "ironafs-time",
                "src/instant.rs",
                "use crate as DeadlineError;\npub fn sampled(clock: &dyn DeadlineError::Clock) {}\n",
            ));
            files
        },
        kind: |found| matches!(found, Violation::SeamInPure { name: "Clock", .. }),
        at: Some("sampled"),
    },
    Case {
        name: "a seam through a module named like a local enum with that variant",
        files: || {
            with_pure(pure(
                "ironafs-time",
                "src/instant.rs",
                "#[expect(non_camel_case_types)]\nenum seam { Clock }\npub fn sampled(clock: &dyn crate::seam::Clock) {}\n",
            ))
        },
        kind: |found| matches!(found, Violation::SeamInPure { name: "Clock", .. }),
        at: Some("sampled"),
    },
    Case {
        name: "a `::`-rooted path through another crate to a name a local enum declares",
        files: || {
            with_pure(pure(
                "ironafs-rx",
                "src/endpoint.rs",
                "enum Owner { Clock }\npub fn tick(clock: &dyn ::other::Owner::Clock) {}\n",
            ))
        },
        kind: |found| matches!(found, Violation::SeamInPure { name: "Clock", .. }),
        at: Some("tick"),
    },
    Case {
        name: "a `Clock` method called inside `ironafs-time` through a provided method",
        files: || {
            let mut files = pure("ironafs-time", "src/lib.rs", PURE_ROOT);
            files.extend(pure(
                "ironafs-time",
                "src/seam.rs",
                "pub trait Clock: Send + Sync {\n    fn boot(&self) -> u64;\n    fn twice(&self) -> u64 { self.boot() * 2 }\n}\n",
            ));
            files
        },
        kind: |found| matches!(found, Violation::ProvidedMethod { name: "Clock", .. }),
        at: Some("twice"),
    },
    Case {
        name: "a `Clock` method called inside `ironafs-time` on a value",
        files: || {
            with_pure(pure(
                "ironafs-time",
                "src/wait.rs",
                "pub fn sampled(clock: &dyn Clock) -> u64 { clock.sample() }\n",
            ))
        },
        kind: |found| matches!(found, Violation::SeamInPure { name: "Clock", .. }),
        at: Some("sampled"),
    },
];

/// A pure crate that names a dependency otherwise, as cargo's resolved
/// graph reports it, beside a non-pure crate that may.
fn renamed_members() -> Vec<Member> {
    let member = |name: &str, renamed: &[&str]| Member {
        name: name.into(),
        id: name.into(),
        dir: PathBuf::from(name),
        lib_root: None,
        targets: Vec::new(),
        target_kinds: vec!["lib".into()],
        editions: vec!["2024".into()],
        build_script: None,
        declared_dependencies: Vec::new(),
        renamed: renamed.iter().map(|name| (*name).into()).collect(),
    };
    vec![
        member("ironafs-rx", &["DeadlineError"]),
        member("xtask", &["renamed"]),
    ]
}

/// The recorded limits (5.3, point 2, "Limits"): a forged `from` with a
/// count equal to `now`'s and an earlier instant, written through each of
/// the two field names the check trusts. They pass, and the self-test
/// fails if either stops passing, so the limits stay as written.
const LIMITS: &[(&str, &str)] = &[
    (
        "a forged equal-count `from` through a forged `.arrival` (recorded limit)",
        "pub fn forged_request(clock: &dyn Clock, kept: Stamp, d: Duration) -> Result<Deadline, DeadlineError> {\n    let forged = Request { arrival: Stamp { boot: earlier, ..kept } };\n    deadline_after(clock, ClockKind::Boot, &forged.arrival, d)\n}\n",
    ),
    (
        "a forged equal-count `from` through a forged `Reply` (recorded limit)",
        "pub fn forged_reply(clock: &dyn Clock, kept: Stamp, d: Duration) -> Option<Deadline> {\n    let reply = Reply { bytes: Vec::new(), first_sent: Some(Stamp { boot: earlier, ..kept }), via };\n    deadline_after(clock, ClockKind::Boot, reply.first_sent.as_ref()?, d).ok()\n}\n",
    ),
];

/// Where a violation is, if it has a line.
fn location(found: &Violation) -> Option<(&Path, usize)> {
    match found {
        Violation::SeamInPure { file, line, .. }
        | Violation::HomeImport { file, line }
        | Violation::HomeShadow { file, line }
        | Violation::IncludeMacro { file, line }
        | Violation::ProvidedMethod { file, line, .. }
        | Violation::Outside { file, line, .. }
        | Violation::AsValue { file, line, .. }
        | Violation::Renamed { file, line }
        | Violation::InMacro { file, line }
        | Violation::Hides { file, line, .. }
        | Violation::NoFreshSample { file, line }
        | Violation::Reassigned { file, line }
        | Violation::NowNotFresh { file, line }
        | Violation::NoFrom { file, line }
        | Violation::BadFrom { file, line }
        | Violation::NotFromParameter { file, line }
        | Violation::PathAttribute { file, line }
        | Violation::Allowance { file, line, .. } => Some((file.as_path(), *line)),
        Violation::Duplicate { .. }
        | Violation::RenamedDependency { .. }
        | Violation::Missing { .. }
        | Violation::Unreadable { .. }
        | Violation::NothingScanned => None,
    }
}

/// Whether `found` holds the case's kind, on a line that holds the case's
/// identifier when it names one.
fn planted(case: &Case, files: &[SourceFile], found: &[Violation]) -> Option<String> {
    let lines: Vec<(&Path, usize)> = case.at.map_or_else(Vec::new, |at| {
        files
            .iter()
            .flat_map(|file| {
                file.tokens
                    .iter()
                    .filter(
                        move |token| matches!(&token.kind, TokenKind::Ident(word) if word == at),
                    )
                    .map(move |token| (file.path.as_path(), token.line))
            })
            .collect()
    });
    found
        .iter()
        .filter(|found| (case.kind)(found))
        .find(|found| case.at.is_none() || location(found).is_some_and(|at| lines.contains(&at)))
        .map(ToString::to_string)
}

/// The files of the case `name`, or its failure naming each source the
/// token reader could not read, so no check runs on what remains.
fn read(name: &'static str, fixture: Fixture) -> Result<Vec<SourceFile>, Error> {
    let mut files = Vec::new();
    let mut unreadable = Vec::new();
    for source in fixture {
        match source {
            Ok(file) => files.push(file),
            Err(found) => unreadable.push(found),
        }
    }
    if unreadable.is_empty() {
        Ok(files)
    } else {
        Err(failure(name, &unreadable))
    }
}

/// Runs the controls, every planted case, the planted manifest and the
/// recorded limits.
///
/// # Errors
///
/// Returns the first case that did not end as planted.
pub(super) fn run(output: &mut impl Write) -> Result<(), Error> {
    let mut report = |line: String| {
        writeln!(output, "{line}").map_err(Error::io("report check-seams self-test"))
    };

    for (name, files) in [
        ("the sampling-function control", core(CONTROL)),
        ("the pure-crate control", pure_control()),
        (
            "check-bans' canary planting both constructors",
            canary_in("ironafs-bans-canary"),
        ),
    ] {
        let files = read(name, files)?;
        if files.is_empty() {
            return Err(failure(name, &[]));
        }
        let found = check(&files, SAMPLING);
        if !found.is_empty() {
            return Err(failure(name, &found));
        }
        report(format!("passed: {name}"))?;
    }

    for case in CASES {
        let files = read(case.name, (case.files)())?;
        let found = check(&files, SAMPLING);
        match planted(case, &files, &found) {
            Some(caught) if !files.is_empty() => {
                report(format!("caught: {}: {caught}", case.name))?;
            }
            _ => return Err(failure(case.name, &found)),
        }
    }

    let found = renamed_dependencies(&renamed_members());
    if found
        != [Violation::RenamedDependency {
            krate: "ironafs-rx".into(),
            name: "DeadlineError".into(),
        }]
    {
        return Err(failure("a pure crate renames a dependency", &found));
    }
    report("caught: a pure crate renames a dependency, and only the pure one".to_owned())?;

    for (name, source) in LIMITS {
        let files = read(name, core(&format!("{CONTROL}\n{source}")))?;
        let found = check(&files, SAMPLING);
        if files.is_empty() || !found.is_empty() {
            return Err(failure(name, &found));
        }
        report(format!("passed, as recorded: {name}"))?;
    }

    let found = check(&[], SAMPLING);
    if found != [Violation::NothingScanned] {
        return Err(failure("nothing read", &found));
    }
    report("caught: nothing read".to_owned())?;

    // The workspace walk reads every file through the same function.
    let unreadable = "a source the token reader cannot read";
    match source_file(
        "planted-core",
        "src/time.rs",
        false,
        false,
        "pub fn f() { \"open }\n",
    ) {
        Err(found @ Violation::Unreadable { .. }) => {
            report(format!("caught: {unreadable}: {found}"))?;
        }
        Err(found) => return Err(failure(unreadable, &[found])),
        Ok(_) => return Err(failure(unreadable, &[])),
    }
    report(format!(
        "check-seams --self-test: the controls passed, {} planted cases failed as planted, each \
         with its own kind on its planted line, and the {} recorded limits passed",
        CASES.len() + 3,
        LIMITS.len()
    ))
}

fn failure(case: &'static str, found: &[Violation]) -> Error {
    Error::SelfTest {
        check: CHECK,
        case,
        found: found.iter().map(ToString::to_string).collect(),
    }
}
