// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The target gate (ADR 0005, point 5.3 and Decision 4): of the targets
//! Cargo selects by a built-in triple, the crate builds for the seven of
//! point 5.3 only, and every other one is refused here, before the crate is
//! compiled. Some built-in targets have exactly the cfg set of a listed one,
//! so the crate root's `compile_error!` guard, kept as the second net,
//! cannot tell them apart; their triple can.
//!
//! Standard library only: no dependency, no other program run, nothing
//! compiled or written, and no variable read but `TARGET`. A custom target
//! named after a listed triple, which Cargo accepts only with unstable
//! target support enabled, is outside what this gate can refuse (point
//! 5.3).

use std::ffi::OsString;
use std::process::ExitCode;

/// The targets of ADR 0005, point 5.3. `cargo xtask check-os` compares
/// this list with its own copy of the point's list.
const TARGETS: [&str; 7] = [
    "x86_64-unknown-linux-gnu",
    "aarch64-unknown-linux-gnu",
    "x86_64-unknown-linux-musl",
    "aarch64-unknown-linux-musl",
    "aarch64-apple-darwin",
    "x86_64-pc-windows-msvc",
    "aarch64-pc-windows-msvc",
];

fn main() -> ExitCode {
    // A new `TARGET` is a new build in any case; while the script holds the
    // gate alone, nothing else it reads can change.
    println!("cargo::rerun-if-changed=build.rs");
    match refusal(std::env::var_os("TARGET")) {
        None => ExitCode::SUCCESS,
        Some(refusal) => {
            println!("cargo::error={refusal}");
            ExitCode::FAILURE
        }
    }
}

/// Why `target` is refused, on one line, or `None` if it is listed. The
/// value is never printed raw: it can be the stem of a target
/// specification file's name, and a raw line break in it would let the
/// rest become a Cargo directive. A UTF-8 value is printed in Rust's debug
/// form, which escapes every line break and control character; an absent
/// or non-UTF-8 one is named, with none of its bytes.
fn refusal(target: Option<OsString>) -> Option<String> {
    let shown = match target.map(OsString::into_string) {
        Some(Ok(target)) if TARGETS.contains(&target.as_str()) => return None,
        Some(Ok(target)) => format!("{target:?}"),
        Some(Err(_)) => "not UTF-8".to_owned(),
        None => "unset".to_owned(),
    };
    Some(format!(
        "ironafs-os: the target gate refuses TARGET {shown}: the crate builds only for the \
         targets of ADR 0005, point 5.3"
    ))
}
