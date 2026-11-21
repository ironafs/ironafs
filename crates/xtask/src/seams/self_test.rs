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

use super::{CHECK, Sampling, SourceFile, Violation, check, source_file};
use crate::Error;

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
const PURE_RX: &str = "pub enum Failure { Clock(u8), Rng }\npub fn failed() -> Failure { Failure::Clock(0) }\npub trait Rpc: Send + Sync { fn call(&self, request: &[u8]); }\npub fn close(endpoint: &mut Endpoint) { endpoint.close(); }\n";

fn core(source: &str) -> Vec<SourceFile> {
    source_file("planted-core", "src/time.rs", false, false, source)
        .map_or_else(|_| Vec::new(), |file| vec![file])
}

fn pure(krate: &str, path: &str, source: &str) -> Vec<SourceFile> {
    source_file(krate, path, true, path == "src/lib.rs", source)
        .map_or_else(|_| Vec::new(), |file| vec![file])
}

fn pure_control() -> Vec<SourceFile> {
    let mut files = pure("ironafs-time", "src/lib.rs", PURE_ROOT);
    files.extend(pure("ironafs-time", "src/seam.rs", PURE_SEAM));
    files.extend(pure("ironafs-rx", "src/rpc.rs", PURE_RX));
    files
}

fn with_pure(extra: Vec<SourceFile>) -> Vec<SourceFile> {
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

/// One planted case: its sources and the violation it must give.
struct Case {
    name: &'static str,
    files: fn() -> Vec<SourceFile>,
    expect: fn(&Violation) -> bool,
}

const CASES: &[Case] = &[
    Case {
        name: "a `now` kept from an earlier sample",
        files: || {
            core(&with_body(
                "ClockKind::Boot => { let kept = now; Deadline::boot_after(&kept, from, d) }",
            ))
        },
        expect: |found| {
            matches!(
                found,
                Violation::NowNotFresh { .. } | Violation::Reassigned { .. }
            )
        },
    },
    Case {
        name: "a `now` passed in, with no fresh sample",
        files: || {
            core(&CONTROL.replace(
                "let now = clock.sample().map_err(DeadlineError::Clock)?;",
                "let now = clock.kept();",
            ))
        },
        expect: |found| {
            matches!(
                found,
                Violation::NoFreshSample { .. } | Violation::NowNotFresh { .. }
            )
        },
    },
    Case {
        name: "the fresh sample's binding reassigned",
        files: || {
            core(&CONTROL.replace(
                "let now = clock.sample().map_err(DeadlineError::Clock)?;",
                "let now = clock.sample().map_err(DeadlineError::Clock)?; now = earlier;",
            ))
        },
        expect: |found| matches!(found, Violation::Reassigned { .. }),
    },
    Case {
        name: "a constructor called outside the sampling function",
        files: || {
            core(&format!(
                "{CONTROL}\npub fn other(now: &Stamp, d: Duration) -> Result<Deadline, DeadlineError> {{\n    Deadline::awake_after(now, now, d)\n}}\n"
            ))
        },
        expect: |found| matches!(found, Violation::Outside { .. }),
    },
    Case {
        name: "a second function in the same crate with its own allowance",
        files: || {
            core(&format!(
                "{CONTROL}\n#[expect(clippy::disallowed_methods, reason = \"a second\")]\npub fn second(clock: &dyn Clock, from: &Stamp, d: Duration) -> Result<Deadline, DeadlineError> {{\n    let now = clock.sample().map_err(DeadlineError::Clock)?;\n    Deadline::boot_after(&now, from, d)\n}}\n"
            ))
        },
        expect: |found| {
            matches!(
                found,
                Violation::Outside { .. }
                    | Violation::Allowance {
                        on: "another function",
                        ..
                    }
            )
        },
    },
    Case {
        name: "a module-level allowance",
        files: || {
            core(&format!(
                "#![expect(clippy::disallowed_methods, reason = \"the module\")]\n{CONTROL}"
            ))
        },
        expect: |found| {
            matches!(
                found,
                Violation::Allowance {
                    on: "a module or the crate",
                    ..
                }
            )
        },
    },
    Case {
        name: "an allowance on a module item",
        files: || {
            core(&format!(
                "{CONTROL}\n#[expect(clippy::disallowed_methods, reason = \"the module\")]\nmod time;\n"
            ))
        },
        expect: |found| matches!(found, Violation::Allowance { on: "a module", .. }),
    },
    Case {
        name: "the constructor carried out by a closure",
        files: || {
            core(&with_body(
                "ClockKind::Boot => { let build = |x: Duration| Deadline::boot_after(&now, from, x); build(d) }",
            ))
        },
        expect: |found| matches!(found, Violation::Hides { .. }),
    },
    Case {
        name: "the constructor carried out by a function pointer",
        files: || {
            core(&with_body(
                "ClockKind::Boot => { let build = Deadline::boot_after; build(&now, from, d) }",
            ))
        },
        expect: |found| matches!(found, Violation::AsValue { .. }),
    },
    Case {
        name: "the constructor inside a macro call",
        files: || {
            core(&with_body(
                "ClockKind::Boot => identity!(Deadline::boot_after(&now, from, d)),",
            ))
        },
        expect: |found| matches!(found, Violation::InMacro { .. }),
    },
    Case {
        name: "a nested function in the sampling function",
        files: || {
            core(&with_body(
                "ClockKind::Boot => { fn inner() {} Deadline::boot_after(&now, from, d) }",
            ))
        },
        expect: |found| matches!(found, Violation::Hides { .. }),
    },
    Case {
        name: "a parameter named `now`, used before the fresh sample",
        files: || {
            core(
                &CONTROL
                    .replace("    kind: ClockKind,\n", "    kind: ClockKind,\n    now: &Stamp,\n")
                    .replace(
                        "    let now = clock.sample().map_err(DeadlineError::Clock)?;\n    match kind {\n        ClockKind::Boot => Deadline::boot_after(&now, from, d),",
                        "    let early = Deadline::boot_after(&now, from, d);\n    let now = clock.sample().map_err(DeadlineError::Clock)?;\n    match kind {\n        ClockKind::Boot => early,",
                    )
                    .replace("ClockKind::Boot, &request.arrival", "ClockKind::Boot, &kept, &request.arrival")
                    .replace("ClockKind::Boot, reply.first_sent", "ClockKind::Boot, &kept, reply.first_sent"),
            )
        },
        expect: |found| {
            matches!(
                found,
                Violation::Reassigned { .. } | Violation::NowNotFresh { .. }
            )
        },
    },
    Case {
        name: "a pattern binding that shadows the fresh sample",
        files: || {
            core(&with_body(
                "ClockKind::Boot => { if let Some(now) = kept { return Deadline::boot_after(&now, from, d); } Deadline::boot_after(&now, from, d) }",
            ))
        },
        expect: |found| matches!(found, Violation::Reassigned { .. }),
    },
    Case {
        name: "a constructor whose `from` is another parameter",
        files: || {
            core(&with_body(
                "ClockKind::Boot => Deadline::boot_after(&now, forged, d),",
            ))
        },
        expect: |found| matches!(found, Violation::NotFromParameter { .. }),
    },
    Case {
        name: "a helper in `ironafs-time` that builds a deadline",
        files: || {
            source_file(
                "ironafs-time",
                "src/wait.rs",
                true,
                false,
                "impl Deadline {\n    pub fn boot_after(now: &Stamp, from: &Stamp, d: Duration) -> Result<Self, DeadlineError> { build() }\n}\npub fn deadline(now: &Stamp, from: &Stamp, d: Duration) -> Result<Deadline, DeadlineError> { Deadline::boot_after(now, from, d) }\n",
            )
            .map_or_else(|_| Vec::new(), |file| vec![file])
        },
        expect: |found| matches!(found, Violation::Outside { .. }),
    },
    Case {
        name: "a `#[path]` module, out of the walk's sight",
        files: || {
            core(&format!(
                "{CONTROL}\n#[path = \"../elsewhere.rs\"]\nmod elsewhere;\n"
            ))
        },
        expect: |found| matches!(found, Violation::PathAttribute { .. }),
    },
    Case {
        name: "a seam through an uppercase alias of its home crate",
        files: || {
            with_pure(pure(
                "ironafs-rx",
                "src/endpoint.rs",
                "use ironafs_time as Time;\npub fn tick(clock: &dyn Time::Clock) -> u64 { 0 }\n",
            ))
        },
        expect: |found| {
            matches!(
                found,
                Violation::SeamInPure { name: "Clock", .. } | Violation::HomeImport { .. }
            )
        },
    },
    Case {
        name: "a seam through a glob re-export behind an uppercase module",
        files: || {
            with_pure(pure(
                "ironafs-cache",
                "src/plan.rs",
                "mod Time { pub use ironafs_time::*; }\npub fn plan(rng: &dyn Time::Rng) {}\n",
            ))
        },
        expect: |found| {
            matches!(
                found,
                Violation::SeamInPure { name: "Rng", .. } | Violation::HomeImport { .. }
            )
        },
    },
    Case {
        name: "a `from` that is a fresh sample",
        files: || core(&with_from("&clock.sample()?")),
        expect: |found| matches!(found, Violation::BadFrom { .. }),
    },
    Case {
        name: "a `from` that is a forged `Stamp` literal with a high count",
        files: || {
            core(&with_from(
                "&Stamp { boot, awake, wall, doubt: Doubt { boot: u64::MAX, awake: u64::MAX }, episodes: 0 }",
            ))
        },
        expect: |found| matches!(found, Violation::BadFrom { .. }),
    },
    Case {
        name: "a `from` that is a local",
        files: || core(&with_from("&arrived")),
        expect: |found| matches!(found, Violation::BadFrom { .. }),
    },
    Case {
        name: "the sampling function renamed on import",
        files: || {
            core(&format!(
                "{CONTROL}\nuse crate::time::deadline_after as make;\n"
            ))
        },
        expect: |found| matches!(found, Violation::Renamed { .. }),
    },
    Case {
        name: "the sampling function passed as a value",
        files: || {
            core(&format!(
                "{CONTROL}\npub fn table() -> usize {{ let f = deadline_after; 0 }}\n"
            ))
        },
        expect: |found| matches!(found, Violation::AsValue { .. }),
    },
    Case {
        name: "the sampling function defined twice",
        files: || {
            let mut files = core(CONTROL);
            files.extend(core(CONTROL));
            files
        },
        expect: |found| matches!(found, Violation::Duplicate { .. }),
    },
    Case {
        name: "the listed sampling function missing from its crate",
        files: || core("pub fn other() {}\n"),
        expect: |found| matches!(found, Violation::Missing { .. }),
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
        expect: |found| matches!(found, Violation::SeamInPure { name: "Clock", .. }),
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
        expect: |found| matches!(found, Violation::SeamInPure { name: "Disk", .. }),
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
        expect: |found| matches!(found, Violation::SeamInPure { name: "Rng", .. }),
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
        expect: |found| matches!(found, Violation::SeamInPure { name: "Clock", .. }),
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
        expect: |found| matches!(found, Violation::SeamInPure { name: "Disk", .. }),
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
        expect: |found| matches!(found, Violation::SeamInPure { name: "Rpc", .. }),
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
        expect: |found| matches!(found, Violation::SeamInPure { name: "Clock", .. }),
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
        expect: |found| matches!(found, Violation::ProvidedMethod { name: "Clock", .. }),
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
        expect: |found| matches!(found, Violation::SeamInPure { name: "Clock", .. }),
    },
];

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

/// Runs the controls, every planted case and the recorded limits.
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
    ] {
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
        let files = (case.files)();
        let found = check(&files, SAMPLING);
        // The planted violation must be among those found; the stricter
        // rules may report the same plant from another side too.
        match found.iter().find(|found| (case.expect)(found)) {
            Some(first) if !files.is_empty() => {
                report(format!("caught: {}: {first}", case.name))?;
            }
            _ => return Err(failure(case.name, &found)),
        }
    }

    for (name, source) in LIMITS {
        let files = core(&format!("{CONTROL}\n{source}"));
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
    report(format!(
        "check-seams --self-test: the controls passed, {} planted cases failed as planted, and \
         the {} recorded limits passed",
        CASES.len() + 1,
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
