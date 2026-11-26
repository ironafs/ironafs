// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Isolated Cargo builds and the internal rustc wrapper that witnesses them.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::{OsStr, OsString};
use std::fs::{self, File};
use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::process::{self, Output};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::json::{self, Value};
use crate::os::pin::{self, Pinned};
use crate::{Command, Error};

use super::options::{self, Invocation};
use super::{CHECK, HARNESS};

#[cfg(test)]
mod tests;

/// Cargo passes this to the internal wrapper along with rustc's argv.
pub(crate) const SESSION_ENV: &str = "__IRONAFS_WIPE_CAPTURE";
const SESSION_FILE: &str = "session";
const MAX_RECORD: usize = 256 * 1024;
const STOP_STATUS: i32 = 86;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Stage {
    Library,
    Assembly,
    Binary,
}

impl Stage {
    fn name(self) -> &'static str {
        match self {
            Self::Library => "library",
            Self::Assembly => "assembly",
            Self::Binary => "binary",
        }
    }

    fn read(name: &str) -> Option<Self> {
        [Self::Library, Self::Assembly, Self::Binary]
            .into_iter()
            .find(|stage| stage.name() == name)
    }
}

/// The tools used by all three builds on a target.
pub(super) struct Tools {
    pub(super) pinned: Pinned,
    cargo: PathBuf,
    wrapper: PathBuf,
    verified_compiler: bool,
}

impl Tools {
    pub(super) fn resolve(root: &Path) -> Result<Self, Error> {
        let mut pinned = pin::resolve(root)?.map_err(refusal)?;
        pinned.rustc = canonical(&pinned.rustc)?;
        let rustc_version = pin::verbose_version(root, &pinned.rustc)?;
        let release =
            pin::release(&rustc_version).ok_or(Error::UnexpectedOutput("pinned rustc release"))?;
        let rustup_rustc = canonical(&pin::rustup_path(root, release, "rustc")?)?;
        if rustup_rustc != pinned.rustc {
            return Err(refusal(
                "the selected rustc is outside the pinned rustup toolchain",
            ));
        }
        let cargo = canonical(&pin::rustup_path(root, release, "cargo")?)?;
        if pin::release(&pin::verbose_version(root, &cargo)?) != Some(release) {
            return Err(refusal("the canonical Cargo and rustc releases differ"));
        }
        let wrapper = canonical(&std::env::current_exe().map_err(Error::io("locate xtask"))?)?;
        let verified_compiler = release == options::DEFAULTS_RELEASE
            && rustc_version
                .lines()
                .any(|line| line.strip_prefix("commit-hash: ") == Some(options::DEFAULTS_COMMIT));
        Ok(Self {
            pinned,
            cargo,
            wrapper,
            verified_compiler,
        })
    }

    /// A fresh target and intermediate build directory for this Cargo command.
    pub(super) fn build(
        &self,
        root: &Path,
        dir: &Path,
        target: &str,
        stage: Stage,
        configure: impl FnOnce(&mut Command),
    ) -> Result<BTreeMap<String, Invocation>, Error> {
        fs::create_dir_all(dir).map_err(Error::path_io("create wipe build parent", dir))?;
        let dir = canonical(dir)?.join(stage.name());
        create_dir(&dir)?;
        let target_dir = dir.join("target");
        let build_dir = dir.join("build");
        create_dir(&target_dir)?;
        create_dir(&build_dir)?;
        let session = Session::create(&build_dir, &self.pinned.rustc, stage, target)?;

        let mut command = Command::new(&self.cargo);
        command.arg(if stage == Stage::Assembly {
            "rustc"
        } else {
            "build"
        });
        command.current_dir(root).args([
            "--release",
            "--offline",
            "--color",
            "never",
            "--message-format=json",
            "--target",
            target,
        ]);
        pin::pin_cargo(&mut command, &self.pinned);
        crate::bans::isolated_build(&mut command, &target_dir)?;
        // Both paths share the parent whose braces isolated_build checked.
        command
            .env("CARGO_BUILD_BUILD_DIR", &build_dir)
            .env("RUSTC_WRAPPER", &self.wrapper)
            .env(SESSION_ENV, &session.dir);
        configure(&mut command);

        let output = command
            .output()
            .map_err(Error::io("run pinned Cargo for wipe"))?;
        let mut artifacts = cargo_evidence(&output, &session, &[&target_dir, &build_dir])?;
        let mut invocations = BTreeMap::new();
        for argv in session.records()? {
            let invocation = options::classify(
                &argv,
                target,
                &[&target_dir, &build_dir],
                self.verified_compiler,
            )
            .map_err(refusal)?;
            validate_stage(&invocation, stage, target, &[&target_dir, &build_dir])?;
            if invocations
                .insert(invocation.name.clone(), invocation)
                .is_some()
            {
                return Err(refusal("duplicate rustc invocation for a target crate"));
            }
        }

        if stage == Stage::Binary {
            if !invocations.contains_key("afs") {
                return Err(refusal("no afs binary invocation recorded"));
            }
            for lib in invocations
                .values()
                .filter(|invocation| invocation.kind != "bin")
            {
                if !artifacts.remove(&library_path(lib)) {
                    return Err(refusal("a target dependency has no fresh Cargo artifact"));
                }
            }
            if !artifacts.is_empty() {
                return Err(refusal(
                    "a target dependency has no completed rustc invocation",
                ));
            }
        }
        Ok(invocations)
    }
}

pub(super) fn refusal(problem: impl Into<String>) -> Error {
    Error::Rules {
        check: CHECK,
        violations: vec![problem.into()],
    }
}

fn canonical(path: &Path) -> Result<PathBuf, Error> {
    fs::canonicalize(path).map_err(Error::path_io("resolve wipe path", path))
}

fn create_dir(path: &Path) -> Result<(), Error> {
    fs::create_dir(path).map_err(Error::path_io("create fresh wipe directory", path))
}

struct Session {
    dir: PathBuf,
    rustc: PathBuf,
    stage: Stage,
    target: String,
    marker: String,
}

impl Session {
    fn create(build_dir: &Path, rustc: &Path, stage: Stage, target: &str) -> Result<Self, Error> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let number = NEXT.fetch_add(1, Ordering::Relaxed);
        let leaf = format!("capture-{}-{number}", process::id());
        let dir = build_dir.join(&leaf);
        create_dir(&dir)?;
        let compiler = rustc.to_str().ok_or(invalid_session())?;
        let fields = [compiler, stage.name(), target];
        let path = dir.join(SESSION_FILE);
        fs::write(&path, fields.join("\0"))
            .map_err(Error::path_io("write wipe capture session", &path))?;
        Ok(Self {
            dir,
            rustc: rustc.to_path_buf(),
            stage,
            target: target.to_owned(),
            marker: format!("error: ironafs-wipe-stop-{leaf}"),
        })
    }

    fn open(dir: &Path) -> Result<Self, Error> {
        let leaf = dir
            .file_name()
            .and_then(OsStr::to_str)
            .ok_or(invalid_session())?;
        let (pid, number) = leaf
            .strip_prefix("capture-")
            .and_then(|leaf| leaf.split_once('-'))
            .ok_or(invalid_session())?;
        if pid.parse::<u32>().is_err()
            || number.parse::<u64>().is_err()
            || !dir.is_absolute()
            || dir.parent().and_then(Path::file_name) != Some(OsStr::new("build"))
            || fs::canonicalize(dir).as_deref().ok() != Some(dir)
        {
            return Err(invalid_session());
        }
        let record = read_record(&dir.join(SESSION_FILE))?;
        let mut fields = record.split('\0');
        let rustc = PathBuf::from(fields.next().ok_or(invalid_session())?);
        let stage = fields
            .next()
            .and_then(Stage::read)
            .ok_or(invalid_session())?;
        let target = fields
            .next()
            .filter(|target| options::TARGETS.contains(target))
            .ok_or(invalid_session())?;
        if fields.next().is_some()
            || !rustc.is_absolute()
            || std::env::var_os("RUSTC").as_deref() != Some(rustc.as_os_str())
            || rustc.file_name()
                != Some(OsStr::new(&format!(
                    "rustc{}",
                    std::env::consts::EXE_SUFFIX
                )))
            || dir
                .parent()
                .and_then(Path::parent)
                .and_then(Path::file_name)
                != Some(OsStr::new(stage.name()))
        {
            return Err(invalid_session());
        }
        Ok(Self {
            dir: dir.to_path_buf(),
            rustc,
            stage,
            target: target.to_owned(),
            marker: format!("error: ironafs-wipe-stop-{leaf}"),
        })
    }

    fn record(&self, argv: &[OsString]) -> Result<(), Error> {
        let words = argv
            .iter()
            .map(|word| {
                word.to_str()
                    .ok_or(Error::UnexpectedOutput("non-UTF-8 rustc argv"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut record = words.join("\0");
        record.push('\0');
        if record.len() > MAX_RECORD {
            return Err(refusal("rustc argument vector exceeds the capture limit"));
        }
        let path = self.dir.join(format!("{}.argv", process::id()));
        crate::files::write_staged(
            &self.dir,
            "wipe-argv",
            |file| file.write_all(record.as_bytes()),
            |staged| {
                fs::hard_link(staged, &path).map_err(Error::path_io("publish wipe capture", &path))
            },
            |operation, path, error| Error::path_io(operation, path)(error),
        )
    }

    fn records(&self) -> Result<Vec<Vec<String>>, Error> {
        let mut records = Vec::new();
        let action = "list wipe capture";
        for entry in fs::read_dir(&self.dir).map_err(Error::path_io(action, &self.dir))? {
            let entry = entry.map_err(Error::path_io(action, &self.dir))?;
            let name = entry.file_name();
            if name == SESSION_FILE {
                continue;
            }
            if name
                .to_str()
                .and_then(|name| name.strip_suffix(".argv"))
                .and_then(|pid| pid.parse::<u32>().ok())
                .is_none()
                || !entry
                    .file_type()
                    .map_err(Error::path_io(action, &entry.path()))?
                    .is_file()
            {
                return Err(refusal("unexpected file in the wipe capture"));
            }
            let bytes = read_record(&entry.path())?;
            let text = bytes
                .strip_suffix('\0')
                .ok_or(Error::UnexpectedOutput("wipe argv record"))?;
            records.push(text.split('\0').map(str::to_owned).collect());
        }
        Ok(records)
    }
}

fn invalid_session() -> Error {
    Error::UnexpectedOutput("internal wipe capture session")
}

fn read_record(path: &Path) -> Result<String, Error> {
    let metadata = fs::symlink_metadata(path).map_err(Error::path_io("stat wipe capture", path))?;
    if !metadata.is_file() || metadata.len() > MAX_RECORD as u64 {
        return Err(Error::UnexpectedOutput("wipe capture file type or size"));
    }
    let file = File::open(path).map_err(Error::path_io("open wipe capture", path))?;
    let mut bytes = Vec::new();
    file.take((MAX_RECORD + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(Error::path_io("read wipe capture", path))?;
    if bytes.len() > MAX_RECORD {
        return Err(Error::UnexpectedOutput("oversized wipe capture"));
    }
    String::from_utf8(bytes).map_err(|_| Error::UnexpectedOutput("non-UTF-8 wipe capture"))
}

/// Forwards the compiler Cargo supplied, preserving its status and streams.
/// Binary capture is the single exception and deliberately stops before rustc.
pub(crate) fn forward(
    dir: &Path,
    mut argv: impl Iterator<Item = OsString>,
    diagnostics: &mut impl std::io::Write,
) -> Result<i32, Error> {
    let session = Session::open(dir)?;
    let compiler = argv.next().ok_or(invalid_session())?;
    if Path::new(&compiler) != session.rustc || !Path::new(&compiler).is_absolute() {
        return Err(refusal(
            "Cargo did not supply the pinned rustc to the wipe wrapper",
        ));
    }
    let argv: Vec<_> = argv.collect();
    if argv.is_empty() {
        return Err(invalid_session());
    }
    let crate_name = std::env::var_os("CARGO_CRATE_NAME");
    let target_call = crate_name.is_some()
        && argv.first().is_some_and(|arg| arg != "-" && arg != "-vV")
        && argv.iter().any(|arg| {
            arg == "--target" || arg.to_str().is_some_and(|arg| arg.starts_with("--target="))
        });
    let selected = target_call
        && match session.stage {
            Stage::Library => crate_name.as_deref() == Some(OsStr::new("ironafs_os")),
            Stage::Assembly => crate_name
                .as_deref()
                .is_some_and(|name| name == "ironafs_os" || name == HARNESS),
            Stage::Binary => true,
        };
    if selected
        && session.stage == Stage::Binary
        && crate_name.as_deref() == Some(OsStr::new("afs"))
    {
        session.record(&argv)?;
        writeln!(diagnostics, "{}", session.marker)
            .map_err(Error::io("write wipe binary marker"))?;
        return Ok(STOP_STATUS);
    }
    let status = Command::new(compiler)
        .args(&argv)
        .env_remove(SESSION_ENV)
        .status()
        .map_err(Error::io("forward to pinned rustc"))?;
    if let Err(failed) = Error::check_status("pinned rustc", status) {
        return Ok(failed.exit_code());
    }
    if selected {
        session.record(&argv)?;
    }
    Ok(0)
}

fn validate_stage(
    invocation: &Invocation,
    stage: Stage,
    target: &str,
    dirs: &[&Path],
) -> Result<(), Error> {
    let allowed = match stage {
        Stage::Library => invocation.name == "ironafs_os" && invocation.kind == "lib",
        Stage::Assembly => {
            matches!(invocation.name.as_str(), "ironafs_os" | HARNESS) && invocation.kind == "lib"
        }
        Stage::Binary => {
            (invocation.name == "afs" && invocation.kind == "bin")
                || (invocation.name != "afs" && matches!(invocation.kind.as_str(), "lib" | "rlib"))
        }
    };
    if !allowed
        || !dirs
            .iter()
            .any(|dir| options::inside(&invocation.out_dir, &dir.join(target).join("release")))
    {
        return Err(refusal(
            "rustc capture has the wrong crate type or release output directory",
        ));
    }
    Ok(())
}

fn library_path(lib: &Invocation) -> PathBuf {
    lib.out_dir.join(format!("lib{}.rlib", lib.output_stem()))
}

fn metadata_path(lib: &Invocation) -> PathBuf {
    library_path(lib).with_extension("rmeta")
}

pub(super) fn is_extern_of(path: &Path, dependency: &Invocation) -> bool {
    path == library_path(dependency) || path == metadata_path(dependency)
}

/// Ensures that every Rust extern comes from a compiler invocation whose
/// profile was captured in the same build. Cargo can hand the caller the
/// dependency's metadata while codegen is still running.
pub(super) fn check_externs(invocations: &BTreeMap<String, Invocation>) -> Result<(), Error> {
    let libraries: BTreeSet<_> = invocations
        .values()
        .filter(|invocation| invocation.kind != "bin")
        .flat_map(|lib| [library_path(lib), metadata_path(lib)])
        .collect();
    for path in invocations
        .values()
        .flat_map(|invocation| &invocation.externs)
    {
        if !libraries.contains(path) || !path.is_file() {
            return Err(refusal("a rustc extern has no captured library build"));
        }
    }
    Ok(())
}

fn cargo_evidence(
    output: &Output,
    session: &Session,
    dirs: &[&Path],
) -> Result<BTreeSet<PathBuf>, Error> {
    let binary = session.stage == Stage::Binary;
    if binary {
        require_binary_stop(output.status.code(), &output.stderr, &session.marker)?;
    } else if !output.status.success() {
        return Err(refusal(format!(
            "the {} release build failed with {}",
            session.stage.name(),
            output.status
        )));
    }

    cargo_messages(&output.stdout, binary, &session.target, dirs)
}

fn cargo_messages(
    stdout: &[u8],
    binary: bool,
    target: &str,
    dirs: &[&Path],
) -> Result<BTreeSet<PathBuf>, Error> {
    let stdout =
        std::str::from_utf8(stdout).map_err(|_| Error::UnexpectedOutput("Cargo build messages"))?;
    let mut artifacts = BTreeSet::new();
    let mut finished = false;
    for line in stdout.lines() {
        let message =
            json::parse(line).map_err(|_| Error::UnexpectedOutput("Cargo build messages"))?;
        let reason = message.get("reason").and_then(Value::as_str);
        match reason {
            Some("build-finished")
                if !finished && message.get("success") == Some(&Value::Bool(!binary)) =>
            {
                finished = true;
            }
            Some("compiler-message") if !finished => {
                let level = message
                    .get("message")
                    .and_then(|message| message.get("level"))
                    .and_then(Value::as_str);
                if !matches!(level, Some("warning" | "note" | "help")) {
                    return Err(refusal(
                        "Cargo reported a compiler error during wipe capture",
                    ));
                }
            }
            Some("compiler-artifact") if !finished => {
                if binary {
                    collect_artifacts(&message, target, dirs, &mut artifacts)?;
                }
            }
            Some("build-script-executed") if !finished => {}
            _ => return Err(Error::UnexpectedOutput("Cargo build message sequence")),
        }
    }
    if !finished {
        return Err(refusal(
            "Cargo did not report the expected build completion",
        ));
    }
    Ok(artifacts)
}

fn require_binary_stop(code: Option<i32>, stderr: &[u8], marker: &str) -> Result<(), Error> {
    let stderr =
        std::str::from_utf8(stderr).map_err(|_| Error::UnexpectedOutput("Cargo binary stop"))?;
    let mut lines = stderr.lines().map(str::trim);
    let errors: Vec<_> = lines
        .clone()
        .filter(|line| line.starts_with("error"))
        .collect();
    if code != Some(101)
        || errors
            != [
                marker,
                "error: could not compile `ironafs-cli` (bin \"afs\")",
            ]
        || !lines.next_back().is_some_and(|line| {
            line.starts_with("process didn't exit successfully:")
                && line.ends_with(&format!("(exit status: {STOP_STATUS})"))
        })
    {
        return Err(refusal(
            "Cargo did not end solely at the expected afs binary marker",
        ));
    }
    Ok(())
}

fn collect_artifacts(
    message: &Value,
    target: &str,
    dirs: &[&Path],
    artifacts: &mut BTreeSet<PathBuf>,
) -> Result<(), Error> {
    let files = message
        .get("filenames")
        .and_then(Value::as_array)
        .ok_or(Error::UnexpectedOutput("Cargo artifact filenames"))?;
    for file in files {
        let file = Path::new(
            file.as_str()
                .ok_or(Error::UnexpectedOutput("Cargo artifact filename"))?,
        );
        let target_output = dirs
            .iter()
            .any(|dir| options::inside(file, &dir.join(target).join("release")));
        if target_output
            && file.extension() == Some(OsStr::new("rlib"))
            && (message.get("fresh") != Some(&Value::Bool(false))
                || !artifacts.insert(file.to_path_buf()))
        {
            return Err(refusal("stale or duplicate Cargo target artifact"));
        }
    }
    Ok(())
}
