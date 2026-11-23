// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The target gate in the crate's build script (ADR 0005, point 5.3 and
//! Decision 4), proved from its own code.
//!
//! - **Its list.** The string literals of the script's `TARGETS` constant
//!   must equal the reference copy of point 5.3's list, `super::ALLOWLIST`,
//!   which is kept here, apart from the gate, and is never read from it.
//! - **Its verdicts.** The script is compiled alone, as a program, and run
//!   with no variable but `TARGET`: before any shim is compiled, anything
//!   linked or any ABI checked. It must accept each listed triple, print
//!   nothing but its rerun rule, and refuse every other built-in target of
//!   the pinned compiler, an absent `TARGET`, a non-UTF-8 one and one
//!   holding a line break, each with one escaped `cargo::error=` line.
//! - **Through Cargo.** Each target the guard accepts but the list does not
//!   name must be refused by Cargo in the build script, with the gate's
//!   message, not later and not for another reason.

use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};

use crate::pure::CLEARED_ENVIRONMENT;
use crate::rust_source::{TokenKind, ident, matching, punct, tokens_with_strings};
use crate::{Command, Error};

#[cfg(test)]
mod tests;

/// The build script, relative to the crate's directory.
pub(crate) const BUILD_SCRIPT: &str = "build.rs";

/// The start of every refusal the gate prints, after `cargo::error=`.
pub(crate) const GATE_ERROR: &str = "ironafs-os: the target gate refuses TARGET";

/// What the gate prints, and only that, when it accepts a target.
pub(crate) const ACCEPTED: &[u8] = b"cargo::rerun-if-changed=build.rs\n";

/// The triples the build script's `TARGETS` constant lists, in order.
///
/// # Errors
///
/// Returns why the script holds no such constant, or one whose items are
/// not all plain target triples.
pub(crate) fn listed(build_rs: &str) -> Result<Vec<String>, String> {
    let tokens = tokens_with_strings(build_rs).map_err(|error| error.to_string())?;
    let name = (1..tokens.len())
        .find(|&at| {
            ident(tokens.get(at)) == Some("TARGETS")
                && ident(at.checked_sub(1).and_then(|before| tokens.get(before))) == Some("const")
        })
        .ok_or("the build script has no `const TARGETS`")?;
    let equals = (name..tokens.len())
        .find(|&at| punct(tokens.get(at)) == Some('='))
        .ok_or("`const TARGETS` has no value")?;
    let close =
        matching(&tokens, equals + 1, '[', ']').ok_or("`const TARGETS` is not an array literal")?;
    let mut triples = Vec::new();
    for token in tokens.get(equals + 2..close).unwrap_or_default() {
        match &token.kind {
            TokenKind::Str(triple) if is_triple(triple) => triples.push(triple.clone()),
            TokenKind::Punct(',') => {}
            _ => {
                return Err(format!(
                    "line {}: `const TARGETS` holds something other than plain target triples",
                    token.line
                ));
            }
        }
    }
    Ok(triples)
}

/// Whether `text` can only be a target triple as written: lower-case
/// letters, digits, `_` and `-`, so no escape can change what it means.
fn is_triple(text: &str) -> bool {
    !text.is_empty()
        && text.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_' || byte == b'-'
        })
}

/// How the gate's list departs from point 5.3's: each listed triple it
/// lacks, each it names that the list does not, and each it repeats.
pub(crate) fn list_problems(gate: &[String], reference: &[&str]) -> Vec<String> {
    let mut problems: Vec<String> = reference
        .iter()
        .filter(|triple| !gate.iter().any(|found| found == *triple))
        .map(|triple| {
            format!("{triple}: in ADR 0005 point 5.3's list, but not in the build script's TARGETS")
        })
        .collect();
    for (index, triple) in gate.iter().enumerate() {
        if !reference.contains(&triple.as_str()) {
            problems.push(format!(
                "{triple}: in the build script's TARGETS, but not in ADR 0005 point 5.3's list"
            ));
        } else if gate.get(..index).unwrap_or_default().contains(triple) {
            problems.push(format!(
                "{triple}: named twice in the build script's TARGETS"
            ));
        }
    }
    problems
}

/// One run of the gate: whether it exited successfully, and what it wrote
/// to its standard output.
#[derive(Debug)]
pub(crate) struct Run {
    pub(crate) success: bool,
    pub(crate) stdout: Vec<u8>,
}

/// What a run of the gate must show.
#[derive(Debug)]
pub(crate) enum Expected<'a> {
    /// The target is accepted: success, and only the rerun rule.
    Accept,
    /// The target is refused, printed as `shown`, and none of `hidden`'s
    /// bytes appear in the output.
    Refuse { shown: &'a str, hidden: &'a [u8] },
}

/// Why `run` is not what `expected` asks, if it is not.
pub(crate) fn judge(run: &Run, expected: &Expected<'_>) -> Option<String> {
    match expected {
        Expected::Accept if run.success && run.stdout == ACCEPTED => None,
        Expected::Accept => Some(format!(
            "the gate did not accept it with only its rerun rule (success: {}, output: {:?})",
            run.success,
            String::from_utf8_lossy(&run.stdout)
        )),
        Expected::Refuse { shown, hidden } => {
            let refusal = refusal_line(&run.stdout);
            let leaked = !hidden.is_empty()
                && run
                    .stdout
                    .windows(hidden.len())
                    .any(|window| window == *hidden);
            match refusal {
                _ if run.success => Some("the gate accepted it".to_owned()),
                _ if leaked => Some("the gate's output holds the value's raw bytes".to_owned()),
                Some(line) if line.contains(shown) && line.contains("point 5.3") => None,
                _ => Some(format!(
                    "the gate did not refuse it with one {GATE_ERROR} line naming {shown} and \
                     point 5.3, after only its rerun rule (output: {:?})",
                    String::from_utf8_lossy(&run.stdout)
                )),
            }
        }
    }
}

/// The text after `cargo::error=` when `stdout` is exactly the rerun rule
/// followed by one refusal of the gate's, each a line of its own.
fn refusal_line(stdout: &[u8]) -> Option<&str> {
    let text = std::str::from_utf8(stdout).ok()?;
    let rest = text.strip_prefix(std::str::from_utf8(ACCEPTED).ok()?)?;
    let line = rest.strip_suffix('\n')?;
    if line.contains(['\n', '\r']) {
        return None;
    }
    line.strip_prefix("cargo::error=")
        .filter(|refusal| refusal.starts_with(GATE_ERROR))
}

/// The build script compiled alone, as a program.
pub(crate) struct Gate {
    program: PathBuf,
}

impl Gate {
    /// Compiles `source` with the pinned compiler, run from `root`, into
    /// `out_dir`.
    ///
    /// # Errors
    ///
    /// Returns an error if `rustc` cannot be started or the directory
    /// cannot be made; a script that does not compile is `Ok(Err(..))`.
    pub(crate) fn compile(
        root: &Path,
        source: &Path,
        out_dir: &Path,
    ) -> Result<Result<Self, String>, Error> {
        fs::create_dir_all(out_dir).map_err(Error::path_io("create directory", out_dir))?;
        let program = out_dir.join(format!("gate{}", std::env::consts::EXE_SUFFIX));
        let output = Command::new("rustc")
            .current_dir(root)
            .args([
                "--edition",
                "2024",
                "--crate-type",
                "bin",
                "--crate-name",
                "gate",
            ])
            .arg("-o")
            .arg(&program)
            .arg(source)
            .output()
            .map_err(Error::io("run rustc on the build script"))?;
        if output.status.success() {
            Ok(Ok(Self { program }))
        } else {
            Ok(Err(format!(
                "the build script does not compile alone: {}",
                crate::pure::first_error(&output.stderr)
            )))
        }
    }

    /// Runs the gate with `TARGET` set to `target`, or unset, and no other
    /// variable.
    ///
    /// # Errors
    ///
    /// Returns an error if the program cannot be started.
    pub(crate) fn run(&self, target: Option<&OsStr>) -> Result<Run, Error> {
        let mut command = Command::new(&self.program);
        command.env_clear();
        if let Some(target) = target {
            command.env("TARGET", target);
        }
        let output = command.output().map_err(Error::io("run the target gate"))?;
        Ok(Run {
            success: output.status.success(),
            stdout: output.stdout,
        })
    }
}

/// A value that is not UTF-8, with the bytes that must never be printed.
#[cfg(unix)]
pub(crate) fn not_utf8() -> (OsString, &'static [u8]) {
    use std::os::unix::ffi::OsStringExt;

    let bytes: &'static [u8] = b"x86_64-unknown-linux-gnu\xff";
    (OsString::from_vec(bytes.to_vec()), b"\xff")
}

/// A value that is not UTF-8 (an unpaired surrogate), with the bytes that
/// must never be printed: its WTF-8 form.
#[cfg(windows)]
pub(crate) fn not_utf8() -> (OsString, &'static [u8]) {
    use std::os::windows::ffi::OsStringExt;

    (OsString::from_wide(&[0x61, 0xD800]), b"\xed\xa0\x80")
}

/// The gate's verdict on each case `targets` and the special values call
/// for: each listed triple accepted; every other target refused, printed
/// in Rust's debug form; an absent value refused as "unset"; a non-UTF-8
/// one as "not UTF-8" with none of its bytes; and a listed triple with a
/// line break and a Cargo directive after it refused on one line.
///
/// # Errors
///
/// Returns an error if the gate cannot be run.
pub(crate) fn verdict_problems(
    gate: &Gate,
    reference: &[&str],
    targets: &[String],
) -> Result<Vec<(String, String)>, Error> {
    let mut problems = Vec::new();
    let mut check = |name: String, value: Option<&OsStr>, expected: Expected<'_>| {
        gate.run(value).map(|run| {
            if let Some(problem) = judge(&run, &expected) {
                problems.push((name, problem));
            }
        })
    };
    for triple in reference {
        check(
            (*triple).to_owned(),
            Some(OsStr::new(triple)),
            Expected::Accept,
        )?;
    }
    for target in targets
        .iter()
        .filter(|target| !reference.contains(&target.as_str()))
    {
        let shown = format!("{target:?}");
        check(
            target.clone(),
            Some(OsStr::new(target)),
            Expected::Refuse {
                shown: &shown,
                hidden: &[],
            },
        )?;
    }
    check(
        "an absent TARGET".to_owned(),
        None,
        Expected::Refuse {
            shown: "unset",
            hidden: &[],
        },
    )?;
    let (value, hidden) = not_utf8();
    check(
        "a non-UTF-8 TARGET".to_owned(),
        Some(&value),
        Expected::Refuse {
            shown: "not UTF-8",
            hidden,
        },
    )?;
    let broken = "x86_64-unknown-linux-gnu\ncargo::rustc-cfg=planted";
    let shown = format!("{broken:?}");
    check(
        "a TARGET holding a line break".to_owned(),
        Some(OsStr::new(broken)),
        Expected::Refuse {
            shown: &shown,
            hidden: b"\ncargo::rustc-cfg",
        },
    )?;
    Ok(problems)
}

/// Why Cargo's build of the crate in `manifest` for `target` was not
/// refused by the gate in the build script, if it was not: it built, or
/// failed later, or for another reason.
///
/// # Errors
///
/// Returns an error if cargo cannot be started.
pub(crate) fn cargo_problem(
    root: &Path,
    manifest: &Path,
    locked: bool,
    target: &str,
    build_dir: &Path,
) -> Result<Option<String>, Error> {
    let mut command = Command::new("cargo");
    command.current_dir(root).arg("check");
    if locked {
        command.arg("--locked");
    }
    command
        .args(["--lib", "--manifest-path"])
        .arg(manifest)
        .args([
            "--package",
            super::CRATE,
            "--color",
            "never",
            "--target",
            target,
        ])
        .arg("--target-dir")
        .arg(build_dir);
    for name in CLEARED_ENVIRONMENT {
        command.env_remove(name);
    }
    let output = command.output().map_err(Error::io("cargo check"))?;
    if output.status.success() {
        return Ok(Some("Cargo built the crate for it".to_owned()));
    }
    Ok(cargo_refusal(&String::from_utf8_lossy(&output.stderr), target).err())
}

/// Whether Cargo's `stderr` shows the build refused in the crate's build
/// script by the gate's message for `target`, and nothing compiled after.
pub(crate) fn cargo_refusal(stderr: &str, target: &str) -> Result<(), String> {
    let in_script = stderr.lines().any(|line| {
        line.starts_with(&format!(
            "error: failed to run custom build command for `{} ",
            super::CRATE
        ))
    });
    let shown = format!("{target:?}");
    let gate = stderr.lines().map(str::trim).any(|line| {
        line.strip_prefix("cargo::error=").is_some_and(|refusal| {
            refusal.starts_with(GATE_ERROR)
                && refusal.contains(&shown)
                && refusal.contains("point 5.3")
        })
    });
    let later = stderr
        .lines()
        .any(|line| line.starts_with("error[") || line.starts_with("error: could not compile"));
    if in_script && gate && !later {
        Ok(())
    } else {
        Err(format!(
            "Cargo's build did not fail in the build script with the gate's message: {}",
            crate::pure::first_error(stderr.as_bytes())
        ))
    }
}
