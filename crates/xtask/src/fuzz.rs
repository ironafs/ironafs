// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The fuzz step: replays every tracked seed of every fuzz target once,
//! through cargo-fuzz on the fuzz crate's pinned nightly (coding standards
//! rule 20; ADR 0004, O47).
//!
//! A reproducer is never printed: the engine's output is captured and
//! never written out, and a failure is reported by its target and the
//! failing seed's digest, from `git hash-object` (ADR 0004, O30). When it
//! runs given files, the engine writes no artefact of its own.

mod self_test;

use std::fmt;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::fixture;
use crate::metadata;
use crate::{Command, Error};

#[cfg(test)]
mod tests;

const CHECK: &str = "fuzz";

/// The fuzz crate, its manifest and its tracked seeds, relative to the
/// workspace root.
const FUZZ_DIR: &str = "fuzz";
const MANIFEST: &str = "Cargo.toml";
const SEEDS: &str = "seeds";

/// One reason the step did not prove every seed ran clean.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Violation {
    /// The manifest names no fuzz target.
    NoTargets,
    /// A line of the manifest that declares a target, or a `[[bin]]` table,
    /// whose name this reader cannot read: refused, never skipped.
    UnreadableTarget { line: usize },
    /// The manifest leaves Cargo free to find targets it declares nowhere.
    AutoBins,
    /// A target has no tracked seed.
    NoSeeds { target: String },
    /// An entry under a target's seed directory that is not a tracked
    /// seed as git's index holds it: untracked, changed, deleted, or not a
    /// regular file. What would be replayed is not what is tracked.
    UntrackedSeed { target: String, entry: PathBuf },
    /// The engine exited successfully but did not report running a seed.
    NotExecuted { target: String, seed: PathBuf },
    /// The engine reported a failure: only the target and the reproducer's
    /// digest are kept.
    Failed {
        target: String,
        digest: Option<String>,
    },
}

impl fmt::Display for Violation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoTargets => write!(
                formatter,
                "{FUZZ_DIR}/{MANIFEST} names no fuzz target, so nothing was replayed"
            ),
            Self::UnreadableTarget { line } => write!(
                formatter,
                "{FUZZ_DIR}/{MANIFEST}:{line}: a fuzz target whose name this check cannot read, \
                 so it would not be replayed: declare each target as a [[bin]] table with one \
                 line name = \"<crate>_<subject>\""
            ),
            Self::AutoBins => write!(
                formatter,
                "{FUZZ_DIR}/{MANIFEST}: [package] does not set autobins = false, so Cargo could \
                 build a target from src/main.rs or src/bin/ that this check never replays"
            ),
            Self::UntrackedSeed { target, entry } => write!(
                formatter,
                "{target}: {} is not a tracked seed as git's index holds it (untracked, changed, deleted or \
                 not a regular file), so what would be replayed is not what is tracked",
                entry.display()
            ),
            Self::NoSeeds { target } => write!(
                formatter,
                "{target}: no tracked seed under {FUZZ_DIR}/{SEEDS}/{target}/, so nothing was replayed"
            ),
            Self::NotExecuted { target, seed } => write!(
                formatter,
                "{target}: the engine did not report running {}",
                seed.display()
            ),
            Self::Failed { target, digest } => match digest {
                Some(digest) => write!(
                    formatter,
                    "{target}: failed on a seed; reproducer digest {digest} (git hash-object); \
                     the input is never printed: report it privately, as SECURITY.md says"
                ),
                None => write!(
                    formatter,
                    "{target}: failed, and no failing seed could be named by digest"
                ),
            },
        }
    }
}

/// The names of the fuzz targets a hand-written fuzz manifest declares:
/// the `name` of each `[[bin]]` table, in order.
///
/// It reads `name = "…"` or `name = '…'`, with an optional trailing
/// comment, and refuses every other way of declaring a target: a name it
/// cannot read, a `[[bin]]` table without exactly one name, a `bin` key,
/// a spaced or quoted `[[ "bin" ]]` header, and any multi-line string,
/// whose lines it could not tell from keys and headers. A target it
/// skipped would never be replayed while the step still reported green.
///
/// # Errors
///
/// Returns the first line it refuses.
pub(crate) fn targets(manifest: &str) -> Result<Vec<String>, Violation> {
    let mut names = Vec::new();
    // The current `[[bin]]` table: its header's line, and its name if read.
    let mut bin: Option<(usize, Option<String>)> = None;
    for (line, text) in (1..).zip(manifest.lines()) {
        let text = text.trim();
        if text.is_empty() || text.starts_with('#') {
            continue;
        }
        let refused = Violation::UnreadableTarget { line };
        if text.contains("\"\"\"") || text.contains("'''") {
            return Err(refused);
        }
        if text.starts_with('[') {
            close_bin(bin.take(), &mut names)?;
            let header = text.split('#').next().unwrap_or_default().trim();
            let bare: String = header
                .chars()
                .filter(|c| !c.is_whitespace() && *c != '"' && *c != '\'')
                .collect();
            if header == "[[bin]]" {
                bin = Some((line, None));
            } else if bare == "[[bin]]" || bare == "[bin]" {
                return Err(refused);
            }
            continue;
        }
        let key = text.split('=').next().unwrap_or_default().trim();
        let unquoted = key.trim_matches(|c| c == '"' || c == '\'');
        if unquoted == "bin" {
            return Err(refused);
        }
        let Some((_, name)) = &mut bin else {
            continue;
        };
        if unquoted != "name" {
            continue;
        }
        if key != "name" || name.is_some() {
            return Err(refused);
        }
        let value = text
            .split_once('=')
            .map_or("", |(_, value)| value.trim_start());
        *name = Some(quoted_name(value).ok_or(refused)?.to_owned());
    }
    close_bin(bin, &mut names)?;
    Ok(names)
}

/// Records the name of a finished `[[bin]]` table, or refuses a table
/// that had none.
fn close_bin(
    bin: Option<(usize, Option<String>)>,
    names: &mut Vec<String>,
) -> Result<(), Violation> {
    match bin {
        Some((_, Some(name))) => {
            names.push(name);
            Ok(())
        }
        Some((line, None)) => Err(Violation::UnreadableTarget { line }),
        None => Ok(()),
    }
}

/// A non-empty name in a basic string without escapes or a literal string,
/// followed by nothing but an optional comment.
fn quoted_name(value: &str) -> Option<&str> {
    let quote = value.chars().next().filter(|c| *c == '"' || *c == '\'')?;
    let rest = value.get(1..)?;
    let end = rest.find(quote)?;
    let name = rest.get(..end)?;
    let after = rest.get(end + 1..)?.trim_start();
    let clean =
        !name.is_empty() && !name.contains('\\') && (after.is_empty() || after.starts_with('#'));
    clean.then_some(name)
}

/// The targets to replay, or why there are none, or why the manifest
/// could hold one this reader does not see.
pub(crate) fn plan(manifest: &str) -> Result<Vec<String>, Violation> {
    let targets = targets(manifest)?;
    if targets.is_empty() {
        Err(Violation::NoTargets)
    } else if !autobins_off(manifest) {
        Err(Violation::AutoBins)
    } else {
        Ok(targets)
    }
}

/// Whether the manifest's `[package]` table sets `autobins = false`, so
/// that every target Cargo builds is a `[[bin]]` table `targets` reads.
fn autobins_off(manifest: &str) -> bool {
    let mut package = false;
    for text in manifest.lines() {
        let text = text.split('#').next().unwrap_or_default().trim();
        if text.starts_with('[') {
            package = text == "[package]";
        } else if package
            && let Some((key, value)) = text.split_once('=')
            && key.trim() == "autobins"
        {
            return value.trim() == "false";
        }
    }
    false
}

/// The seed the engine was running when it stopped: the last one it
/// reports starting and not finishing.
pub(crate) fn failing_seed<'s>(seeds: &'s [PathBuf], log: &str) -> Option<&'s PathBuf> {
    let last = log
        .lines()
        .rev()
        .find_map(|line| line.strip_prefix("Running: "))?;
    let seed = seeds.iter().find(|seed| seed.as_os_str() == last)?;
    let executed = format!("Executed {last} in ");
    (!log.lines().any(|line| line.starts_with(&executed))).then_some(seed)
}

/// Judges one target's replay from the engine's exit and output: it
/// passes only if the engine succeeded and reported running every seed.
/// A failure keeps the target and the failing seed's digest, from
/// `digest_of`, and nothing of the engine's output.
pub(crate) fn verdict(
    target: &str,
    seeds: &[PathBuf],
    succeeded: bool,
    log: &str,
    digest_of: &dyn Fn(&Path) -> Option<String>,
) -> Result<(), Violation> {
    if !succeeded {
        return Err(Violation::Failed {
            target: target.to_owned(),
            digest: failing_seed(seeds, log).and_then(|seed| digest_of(seed)),
        });
    }
    if seeds.is_empty() {
        return Err(Violation::NoSeeds {
            target: target.to_owned(),
        });
    }
    for seed in seeds {
        let executed = format!("Executed {} in ", seed.display());
        if !log.lines().any(|line| line.starts_with(&executed)) {
            return Err(Violation::NotExecuted {
                target: target.to_owned(),
                seed: seed.clone(),
            });
        }
    }
    Ok(())
}

/// A seed's git object id, which names it without its bytes; `None` if
/// git cannot read it or answers with anything but one hexadecimal id.
pub(crate) fn git_digest(fuzz_dir: &Path, seed: &Path) -> Option<String> {
    let output = git_in(fuzz_dir)
        .args(["hash-object", "--no-filters", "--"])
        .arg(seed)
        .output()
        .ok()?;
    let id = String::from_utf8(output.stdout).ok()?;
    let id = id.trim_end_matches('\n');
    (output.status.success()
        && matches!(id.len(), 40 | 64)
        && id.bytes().all(|byte| byte.is_ascii_hexdigit()))
    .then(|| id.to_owned())
}

/// The tracked seeds of one target, sorted, as paths relative to the fuzz
/// crate: each regular file git's index holds under its seed directory,
/// read in `fuzz_dir`. Any entry there that the working tree holds
/// otherwise (untracked, changed or deleted) or that is not a regular
/// file is refused (`seed_list`).
///
/// # Errors
///
/// Returns an error if git cannot run or answers unexpectedly.
fn seeds(fuzz_dir: &Path, target: &str) -> Result<Result<Vec<PathBuf>, Violation>, Error> {
    let dir = Path::new(SEEDS).join(target);
    let tracked = git_ls_files(fuzz_dir, &["--stage"], &dir)?;
    let stray = git_ls_files(fuzz_dir, &["--others", "--modified"], &dir)?;
    Ok(seed_list(target, &tracked, &stray))
}

/// Git, run in `dir` on the repository that holds it: a hook's exported
/// `GIT_DIR` and its kin would otherwise name another one, or the same one
/// relative to another directory.
pub(crate) fn git_in(dir: &Path) -> Command {
    let mut command = Command::new("git");
    command.current_dir(dir);
    for name in [
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_INDEX_FILE",
        "GIT_COMMON_DIR",
    ] {
        command.env_remove(name);
    }
    command
}

/// `git ls-files -z` with `options`, for the paths under `dir`, relative
/// to `fuzz_dir`, in which it runs.
fn git_ls_files(fuzz_dir: &Path, options: &[&str], dir: &Path) -> Result<Vec<u8>, Error> {
    const COMMAND: &str = "git ls-files";
    let output = git_in(fuzz_dir)
        .args(["ls-files", "-z"])
        .args(options)
        .arg("--")
        .arg(dir)
        .output()
        .map_err(Error::io(COMMAND))?;
    Error::check_status(COMMAND, output.status)?;
    Ok(output.stdout)
}

/// One target's seeds from `git ls-files -z --stage` (`tracked`) and
/// `git ls-files -z --others --modified` (`stray`): each tracked regular
/// file, sorted, when nothing is stray and every tracked entry is a
/// regular file; otherwise the first entry that is not.
pub(crate) fn seed_list(
    target: &str,
    tracked: &[u8],
    stray: &[u8],
) -> Result<Vec<PathBuf>, Violation> {
    let refused = |entry: &str| Violation::UntrackedSeed {
        target: target.to_owned(),
        entry: PathBuf::from(entry),
    };
    let entries = |bytes: &[u8]| -> Vec<String> {
        bytes
            .split(|byte| *byte == 0)
            .filter(|entry| !entry.is_empty())
            .map(|entry| String::from_utf8_lossy(entry).into_owned())
            .collect()
    };
    if let Some(entry) = entries(stray).first() {
        return Err(refused(entry));
    }
    let mut seeds = Vec::new();
    for entry in entries(tracked) {
        // `<mode> <object> <stage>\t<path>`.
        let Some((info, path)) = entry.split_once('\t') else {
            return Err(refused(&entry));
        };
        let regular = matches!(info.split(' ').next(), Some("100644" | "100755"));
        if !regular || !info.ends_with(" 0") {
            return Err(refused(path));
        }
        seeds.push(PathBuf::from(path));
    }
    seeds.sort();
    Ok(seeds)
}

/// A `cargo` command run in the fuzz crate, on its own pinned toolchain.
/// The rustup proxy exports the workspace's toolchain to `cargo xtask`;
/// inherited, it would override `fuzz/rust-toolchain.toml`.
pub(crate) fn in_fuzz_crate(fuzz_dir: &Path) -> Command {
    let mut command = Command::new("cargo");
    command.current_dir(fuzz_dir).env_remove("RUSTUP_TOOLCHAIN");
    command
}

/// Replays one target's seeds, the engine's output captured, never shown.
fn replay(
    fuzz_dir: &Path,
    target: &str,
    seeds: &[PathBuf],
    artefacts: &Path,
) -> Result<Result<(), Violation>, Error> {
    fs::create_dir_all(artefacts)
        .map_err(Error::path_io("create artefact directory", artefacts))?;
    let mut prefix = artefacts.as_os_str().to_owned();
    prefix.push("/");
    let mut artefact_prefix = std::ffi::OsString::from("-artifact_prefix=");
    artefact_prefix.push(&prefix);
    let output = in_fuzz_crate(fuzz_dir)
        .args(["fuzz", "run", target])
        .args(seeds)
        .arg("--")
        .arg(artefact_prefix)
        .output()
        .map_err(Error::io("run cargo fuzz run"))?;
    let mut log = String::from_utf8_lossy(&output.stdout).into_owned();
    log.push_str(&String::from_utf8_lossy(&output.stderr));
    Ok(verdict(
        target,
        seeds,
        output.status.success(),
        &log,
        &|seed| git_digest(fuzz_dir, seed),
    ))
}

/// Replays every seed of every target, or with `self_test` proves on
/// planted manifests, seed sets and engine outputs that the step fails
/// when nothing ran and keeps a reproducer out of its report.
///
/// # Errors
///
/// Returns every violation, or why the step could not run.
pub(super) fn run(self_test: bool, output: &mut impl Write) -> Result<(), Error> {
    let workspace = metadata::load(None, &[])?;
    let fuzz_dir = workspace.root.join(FUZZ_DIR);
    if self_test {
        return self_test::run(&fuzz_dir, &workspace.target_dir, output);
    }
    let manifest_path = fuzz_dir.join(MANIFEST);
    let manifest = fs::read_to_string(&manifest_path)
        .map_err(Error::path_io("read fuzz manifest", &manifest_path))?;
    let targets = plan(&manifest).map_err(|violation| rules(&[violation]))?;
    // The lock file is used as committed: a stale one fails here, before
    // cargo-fuzz, which has no --locked, could rewrite it.
    let status = in_fuzz_crate(&fuzz_dir)
        .args(["--locked", "fetch"])
        .status()
        .map_err(Error::io("run cargo --locked fetch"))?;
    Error::check_status("cargo --locked fetch in fuzz/", status)?;
    // Built first, its output shown: a build reads no input, so a failure
    // here is reported as one, not as a failed replay.
    let status = in_fuzz_crate(&fuzz_dir)
        .args(["fuzz", "build"])
        .status()
        .map_err(Error::io("run cargo fuzz build"))?;
    Error::check_status("cargo fuzz build in fuzz/", status)?;

    let artefacts = fixture::fresh_dir(&workspace.target_dir, CHECK)?;
    // Removed whatever the replay returned, an error included.
    let result = replay_all(&fuzz_dir, &targets, &artefacts, output);
    fixture::remove_dir(&artefacts)?;
    result
}

/// Replays every seed of each of `targets`, each target's artefacts under
/// `artefacts`, and reports.
fn replay_all(
    fuzz_dir: &Path,
    targets: &[String],
    artefacts: &Path,
    output: &mut impl Write,
) -> Result<(), Error> {
    let mut violations = Vec::new();
    let mut replayed = 0_usize;
    for target in targets {
        let seeds = match seeds(fuzz_dir, target)? {
            Ok(seeds) => seeds,
            Err(violation) => {
                violations.push(violation);
                continue;
            }
        };
        if seeds.is_empty() {
            violations.push(Violation::NoSeeds {
                target: target.clone(),
            });
            continue;
        }
        match replay(fuzz_dir, target, &seeds, &artefacts.join(target))? {
            Ok(()) => {
                replayed += seeds.len();
                writeln!(output, "{CHECK}: {target}: {} seeds ran clean", seeds.len())
                    .map_err(Error::io("report fuzz"))?;
            }
            Err(violation) => violations.push(violation),
        }
    }
    if !violations.is_empty() {
        return Err(rules(&violations));
    }
    writeln!(
        output,
        "{CHECK}: {replayed} seeds of {} targets ran clean",
        targets.len()
    )
    .map_err(Error::io("report fuzz"))
}

fn rules(violations: &[Violation]) -> Error {
    Error::Rules {
        check: CHECK,
        violations: violations.iter().map(ToString::to_string).collect(),
    }
}
