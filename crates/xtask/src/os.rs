// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! `check-os`: the `unsafe` of the workspace is where ADR 0005 puts it,
//! and nowhere else.
//!
//! - **Confinement** (point 1.5). No `.rs` file outside `ironafs-os`, in
//!   any crate or anywhere else in the repository, holds an `unsafe`
//!   token, a foreign declaration or function, or a `#[link]` attribute.
//!   (The lint table's own rule is `check-deps`'.)
//! - **Inventory** (point 6.7). `crates/ironafs-os/sites.json` holds every
//!   row of the ADR's table, cell for cell (`inventory`). For each target
//!   of the allowlist (point 5.3) the crate is compiled, and the files the
//!   compiler read for it, from its dependency-info file, are that
//!   target's code. The sites in them (`scan`) must equal, row by row,
//!   what the copy records for each row implemented on that target, and
//!   hold nothing of a row only approved there or of no row. A site must
//!   sit in its row's module and in a file some target compiles.
//! - **Targets.** Every allowlisted target the toolchain names builds; any
//!   other target the toolchain names fails with the crate's allowlist
//!   error. The musl targets are not built here: the copy gives them the
//!   glibc targets' statuses, and the crate names `target_env` only in its
//!   allowlist, so a musl build compiles the glibc build's files.

mod allowlist;
pub(crate) mod inventory;
pub(crate) mod scan;
mod self_test;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::json;
use crate::metadata::{self, Workspace};
use crate::pure::{CLEARED_ENVIRONMENT, first_error, matrix, toolchain_targets};
use crate::rust_source;
use crate::{Command, Error};
use inventory::{Inventory, Row};
use scan::{Scan, SiteKind};

#[cfg(test)]
mod tests;

const CHECK: &str = "check-os";

/// The crate that may hold `unsafe` (ADR 0005, point 1.1).
pub(crate) const CRATE: &str = "ironafs-os";

/// The copy of the ADR's table, relative to the crate's directory.
pub(crate) const SITES: &str = "sites.json";

/// The targets of ADR 0005, point 5.3.
pub(crate) const ALLOWLIST: &[&str] = &[
    "x86_64-unknown-linux-gnu",
    "aarch64-unknown-linux-gnu",
    "x86_64-unknown-linux-musl",
    "aarch64-unknown-linux-musl",
    "aarch64-apple-darwin",
    "x86_64-pc-windows-msvc",
    "aarch64-pc-windows-msvc",
];

/// The text of the crate's allowlist error, which a build for any other
/// target must report.
const ALLOWLIST_ERROR: &str = "ironafs-os builds only for the targets of ADR 0005, point 5.3";

/// Whether the check builds `target` itself: every allowlisted target but
/// musl, whose code is the glibc target's.
fn built_here(target: &str) -> bool {
    ALLOWLIST.contains(&target) && !target.ends_with("-musl")
}

/// For each target, the files (relative to the crate) the compiler read
/// for it.
pub(crate) type Compiled = BTreeMap<String, BTreeSet<PathBuf>>;

/// One file of the crate: its path relative to the crate's directory and
/// what it holds.
#[derive(Debug)]
pub(crate) struct CrateFile {
    pub(crate) path: PathBuf,
    pub(crate) scan: Scan,
}

/// One broken rule.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Violation {
    /// `unsafe`, a foreign item or `#[link]` outside the crate.
    Outside { file: PathBuf, line: usize },
    /// A rule of the site reader, in a file of the crate.
    Rule {
        file: PathBuf,
        line: usize,
        what: &'static str,
    },
    /// A file the token reader could not read.
    Unreadable { file: PathBuf, error: String },
    /// A site with no named expectation.
    Unnamed { file: PathBuf, line: usize },
    /// A site whose id is no row of the table.
    Unknown {
        file: PathBuf,
        line: usize,
        id: String,
    },
    /// A site outside its row's module.
    OutsideModule {
        file: PathBuf,
        line: usize,
        id: String,
        module: String,
    },
    /// A site in a file no target compiles.
    NotCompiled {
        file: PathBuf,
        line: usize,
        id: String,
    },
    /// What a target's code holds for a row differs from the copy.
    Mismatch {
        target: String,
        id: String,
        expected: Vec<String>,
        found: Vec<String>,
    },
    /// A file the compiler read for a target that this check did not scan.
    NotScanned { target: String, file: PathBuf },
    /// The copy departs from the ADR or from its own rules.
    Inventory(String),
    /// A target was not built as the allowlist requires.
    Target { target: String, what: String },
}

impl fmt::Display for Violation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Outside { file, line } => {
                write!(formatter, "{}:{line}: {}", file.display(), scan::OUTSIDE)
            }
            Self::Rule { file, line, what } => {
                write!(formatter, "{}:{line}: {what}", file.display())
            }
            Self::Unreadable { file, error } => write!(
                formatter,
                "{}: not readable as Rust source ({error}), so its sites cannot be counted",
                file.display()
            ),
            Self::Unnamed { file, line } => write!(
                formatter,
                "{}:{line}: a site with no #[expect(unsafe_code, reason = \"<site id>\")] on its \
                 statement or item (ADR 0005, point 6.1)",
                file.display()
            ),
            Self::Unknown { file, line, id } => write!(
                formatter,
                "{}:{line}: site id `{id}` is no row of ADR 0005's table (point 2)",
                file.display()
            ),
            Self::OutsideModule {
                file,
                line,
                id,
                module,
            } => write!(
                formatter,
                "{}:{line}: a `{id}` site outside its module `{module}`, as sites.json records it",
                file.display()
            ),
            Self::NotCompiled { file, line, id } => write!(
                formatter,
                "{}:{line}: a `{id}` site in a file no allowlisted target compiles, so no \
                 target's count sees it",
                file.display()
            ),
            Self::Mismatch {
                target,
                id,
                expected,
                found,
            } => write!(
                formatter,
                "{target}: `{id}` holds [{}] where sites.json records [{}] (ADR 0005, point 6.7)",
                found.join(", "),
                expected.join(", ")
            ),
            Self::NotScanned { target, file } => write!(
                formatter,
                "{target}: the compiler read {}, which this check did not scan, so its sites \
                 went uncounted",
                file.display()
            ),
            Self::Inventory(problem) => write!(formatter, "{problem}"),
            Self::Target { target, what } => write!(formatter, "{target}: {what}"),
        }
    }
}

/// What a row holds, one entry per site, sorted: an extern block with its
/// link and functions, a block with its operation, an impl.
fn describe(kind: &SiteKind) -> String {
    match kind {
        SiteKind::Extern { link, items } => {
            let mut names: Vec<String> = items
                .iter()
                .map(|item| {
                    let safety = if item.safe { "safe fn" } else { "fn" };
                    format!("{safety} {}", item.name)
                })
                .collect();
            names.sort();
            format!(
                "extern block (link {}): {}",
                link.as_deref().unwrap_or("none"),
                names.join(" ")
            )
        }
        SiteKind::Block {
            operation,
            arguments,
        } => format!("block: {operation}({})", arguments.join(", ")),
        SiteKind::Impl => "impl".to_owned(),
        SiteKind::StandInFn => "Miri stand-in function".to_owned(),
    }
}

/// What the copy records for a row: empty when it is not implemented on the
/// target.
fn expected_for(row: &Row, target: &str) -> Vec<String> {
    let Some(code) = row.code.as_ref().filter(|_| row.implemented_on(target)) else {
        return Vec::new();
    };
    let mut kinds: Vec<SiteKind> = code
        .extern_blocks
        .iter()
        .map(|block| SiteKind::Extern {
            link: block.link.clone(),
            items: block.items.clone(),
        })
        .collect();
    kinds.extend(code.blocks.iter().map(|block| SiteKind::Block {
        operation: block.operation.clone(),
        arguments: block.arguments.clone(),
    }));
    kinds.extend((0..code.impls).map(|_| SiteKind::Impl));
    let mut described: Vec<String> = kinds.iter().map(describe).collect();
    described.sort();
    described
}

/// Whether `file` (relative to the crate) is in `module`: `src/<module>.rs`
/// or under `src/<module>/`.
fn in_module(file: &Path, module: &str) -> bool {
    let src = Path::new("src");
    file == src.join(format!("{module}.rs")) || file.starts_with(src.join(module))
}

/// Compares the crate's files with the copy, for each target in
/// `compiled`, which maps a target to the files (relative to the crate)
/// the compiler read for it.
pub(crate) fn compare(
    inventory: &Inventory,
    files: &[CrateFile],
    compiled: &BTreeMap<String, BTreeSet<PathBuf>>,
) -> Vec<Violation> {
    let mut violations = Vec::new();
    for file in files {
        violations.extend(file.scan.problems.iter().map(|problem| Violation::Rule {
            file: file.path.clone(),
            line: problem.line,
            what: problem.what,
        }));
        let compiled_somewhere = compiled.values().any(|set| set.contains(&file.path));
        violations.extend(site_violations(inventory, file, compiled_somewhere));
    }
    violations.extend(stand_in_mismatches(inventory, files));
    let scanned: BTreeSet<&PathBuf> = files.iter().map(|file| &file.path).collect();
    for (target, read) in compiled {
        for path in read.iter().filter(|path| !scanned.contains(path)) {
            violations.push(Violation::NotScanned {
                target: target.clone(),
                file: path.clone(),
            });
        }
        let mut found: BTreeMap<&str, Vec<String>> = BTreeMap::new();
        for file in files.iter().filter(|file| read.contains(&file.path)) {
            for site in &file.scan.sites {
                if let Some(id) = &site.id {
                    // A stand-in is never compiled for a target; if it were,
                    // its sites count, under its own name, and mismatch.
                    found.entry(id).or_default().push(describe(&site.kind));
                }
            }
        }
        for (id, held) in &found {
            if id.ends_with(scan::STAND_IN) {
                violations.push(Violation::Mismatch {
                    target: target.clone(),
                    id: (*id).to_owned(),
                    expected: Vec::new(),
                    found: held.clone(),
                });
            }
        }
        for row in &inventory.rows {
            let expected = expected_for(row, target);
            let mut held = found.remove(row.id.as_str()).unwrap_or_default();
            held.sort();
            if held != expected {
                violations.push(Violation::Mismatch {
                    target: target.clone(),
                    id: row.id.clone(),
                    expected,
                    found: held,
                });
            }
        }
    }
    violations
}

/// The rules each site of one file must meet on its own: a name, a known
/// row (or a stand-in's), its row's module, and a file some target
/// compiles.
fn site_violations(
    inventory: &Inventory,
    file: &CrateFile,
    compiled_somewhere: bool,
) -> Vec<Violation> {
    let mut violations = Vec::new();
    let row_of = |id: &str| inventory.rows.iter().find(|row| row.id == id);
    for site in &file.scan.sites {
        let line = site.line;
        let Some(id) = &site.id else {
            violations.push(Violation::Unnamed {
                file: file.path.clone(),
                line,
            });
            continue;
        };
        if let Some(base) = id.strip_suffix(scan::STAND_IN) {
            violations.extend(stand_in_violations(
                inventory,
                file,
                line,
                base,
                compiled_somewhere,
            ));
            continue;
        }
        if site.kind == SiteKind::StandInFn {
            violations.push(Violation::Rule {
                file: file.path.clone(),
                line,
                what: scan::UNSAFE_ITEM,
            });
        }
        let Some(row) = row_of(id) else {
            violations.push(Violation::Unknown {
                file: file.path.clone(),
                line,
                id: id.clone(),
            });
            continue;
        };
        if let Some(code) = &row.code
            && !in_module(&file.path, &code.module)
        {
            violations.push(Violation::OutsideModule {
                file: file.path.clone(),
                line,
                id: id.clone(),
                module: code.module.clone(),
            });
        }
        if !compiled_somewhere {
            violations.push(Violation::NotCompiled {
                file: file.path.clone(),
                line,
                id: id.clone(),
            });
        }
    }
    violations
}

/// Each row's Miri stand-ins against what the copy records for them: the
/// number of stand-in functions and each block's call and arguments.
fn stand_in_mismatches(inventory: &Inventory, files: &[CrateFile]) -> Vec<Violation> {
    let mut violations = Vec::new();
    for row in &inventory.rows {
        let id = format!("{}{}", row.id, scan::STAND_IN);
        let mut held: Vec<String> = files
            .iter()
            .flat_map(|file| &file.scan.sites)
            .filter(|site| site.id.as_deref() == Some(id.as_str()))
            .map(|site| describe(&site.kind))
            .collect();
        held.sort();
        let mut expected: Vec<String> = row
            .code
            .as_ref()
            .and_then(|code| code.stand_in.as_ref())
            .map_or_else(Vec::new, |stand_in| {
                let mut kinds: Vec<SiteKind> = stand_in
                    .blocks
                    .iter()
                    .map(|(operation, arguments)| SiteKind::Block {
                        operation: operation.clone(),
                        arguments: arguments.clone(),
                    })
                    .collect();
                kinds.extend((0..stand_in.functions).map(|_| SiteKind::StandInFn));
                kinds.iter().map(describe).collect()
            });
        expected.sort();
        if held != expected {
            violations.push(Violation::Mismatch {
                target: "cfg(miri)".to_owned(),
                id,
                expected,
                found: held,
            });
        }
    }
    violations
}

/// The rules for a Miri stand-in's site (ADR 0005, point 6.5): it stands
/// in for a row implemented somewhere, sits in that row's module in a file
/// named `stand_in.rs`, and is compiled for no target (only under Miri).
fn stand_in_violations(
    inventory: &Inventory,
    file: &CrateFile,
    line: usize,
    base: &str,
    compiled_somewhere: bool,
) -> Vec<Violation> {
    let id = format!("{base}{}", scan::STAND_IN);
    let module = inventory
        .rows
        .iter()
        .find(|row| row.id == base)
        .and_then(|row| row.code.as_ref())
        .map(|code| code.module.clone());
    let Some(module) = module else {
        return vec![Violation::Unknown {
            file: file.path.clone(),
            line,
            id,
        }];
    };
    let named = file
        .path
        .file_name()
        .is_some_and(|name| name == "stand_in.rs");
    let mut found = Vec::new();
    if !named || !in_module(&file.path, &module) {
        found.push(Violation::OutsideModule {
            file: file.path.clone(),
            line,
            id: id.clone(),
            module: format!("{module}, in a file named stand_in.rs"),
        });
    }
    if compiled_somewhere {
        found.push(Violation::Rule {
            file: file.path.clone(),
            line,
            what: STAND_IN_COMPILED,
        });
    }
    found
}

/// Why a stand-in compiled for a target is refused.
const STAND_IN_COMPILED: &str = "a Miri stand-in is compiled for an allowlisted target: it must be \
                                 built only under cfg(miri), in place of the foreign function";

/// Every `.rs` file under `dir`, skipping build output, hidden directories
/// and anything `skip` accepts.
fn rust_files(dir: &Path, skip: &dyn Fn(&Path) -> bool) -> Result<Vec<PathBuf>, Error> {
    let mut found = Vec::new();
    let excluded = |path: &Path| {
        skip(path)
            || path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name == "target" || (name.starts_with('.') && path.is_dir()))
    };
    rust_source::files(dir, &excluded, &mut found)?;
    Ok(found)
}

/// The confinement rule over every `.rs` file of the repository outside
/// the crate's directory.
fn outside_violations(root: &Path, crate_dir: &Path) -> Result<Vec<Violation>, Error> {
    let mut violations = Vec::new();
    for file in rust_files(root, &|path| path == crate_dir)? {
        let shown = file.strip_prefix(root).unwrap_or(&file).to_path_buf();
        let text = fs::read_to_string(&file).map_err(Error::path_io("read source file", &file))?;
        match rust_source::tokens_with_strings(&text) {
            Ok(tokens) => {
                violations.extend(scan::outside_crate(&tokens).into_iter().map(|problem| {
                    Violation::Outside {
                        file: shown.clone(),
                        line: problem.line,
                    }
                }));
            }
            Err(error) => violations.push(Violation::Unreadable {
                file: shown,
                error: error.to_string(),
            }),
        }
    }
    Ok(violations)
}

/// Reads and scans the crate's files; a file that is not Rust source is a
/// violation.
pub(crate) fn crate_files(crate_dir: &Path) -> Result<(Vec<CrateFile>, Vec<Violation>), Error> {
    let mut files = Vec::new();
    let mut violations = Vec::new();
    let root = Path::new("src").join("lib.rs");
    let (all, links) = crate::pure::package_files(crate_dir, &crate_dir.join("target"))?;
    for link in links {
        violations.push(Violation::Rule {
            file: link.strip_prefix(crate_dir).unwrap_or(&link).to_path_buf(),
            line: 0,
            what: SYMLINK,
        });
    }
    for file in all
        .into_iter()
        .filter(|file| file.extension().is_some_and(|extension| extension == "rs"))
    {
        let path = file.strip_prefix(crate_dir).unwrap_or(&file).to_path_buf();
        let text = fs::read_to_string(&file).map_err(Error::path_io("read source file", &file))?;
        match rust_source::tokens_with_strings(&text) {
            Ok(tokens) => {
                let scan = scan::scan(&tokens, path == root);
                files.push(CrateFile { path, scan });
            }
            Err(error) => violations.push(Violation::Unreadable {
                file: path,
                error: error.to_string(),
            }),
        }
    }
    Ok((files, violations))
}

/// Why a symbolic link in the crate is refused.
const SYMLINK: &str = "a symbolic link in ironafs-os: the compiler could read through it source \
                       this check does not scan";

/// Builds the crate's library for each target the toolchain names and
/// returns, for each allowlisted target built here, the files the compiler
/// read (relative to the crate), with the violations: an allowlisted
/// target missing from the toolchain, a build that failed or read nothing,
/// and a build for another target that did not fail with the allowlist's
/// error.
pub(crate) fn compiled_files(
    workspace: &Workspace,
    crate_dir: &Path,
    build_dir: &Path,
) -> Result<(Compiled, Vec<Violation>), Error> {
    let toolchain_path = workspace.root.join("rust-toolchain.toml");
    let toolchain = fs::read_to_string(&toolchain_path)
        .map_err(Error::path_io("read toolchain file", &toolchain_path))?;
    let targets = toolchain_targets(&toolchain)?;
    let mut violations: Vec<Violation> = ALLOWLIST
        .iter()
        .filter(|target| built_here(target) && !targets.iter().any(|found| found == *target))
        .map(|target| Violation::Target {
            target: (*target).to_owned(),
            what: "is in ADR 0005's allowlist but not in rust-toolchain.toml, so its sites \
                   are not counted"
                .to_owned(),
        })
        .collect();
    let mut compiled = BTreeMap::new();
    for target in &targets {
        match files_for(workspace, crate_dir, build_dir, target)? {
            Built::Files(files) if built_here(target) => {
                compiled.insert(target.clone(), files);
            }
            Built::Refused(error) if !built_here(target) && error.contains(ALLOWLIST_ERROR) => {}
            Built::Refused(error) if built_here(target) => violations.push(Violation::Target {
                target: target.clone(),
                what: error,
            }),
            _ if ALLOWLIST.contains(&target.as_str()) => {}
            _ => violations.push(Violation::Target {
                target: target.clone(),
                what: "is outside ADR 0005's allowlist, but the crate's build did not fail \
                       with the allowlist's error"
                    .to_owned(),
            }),
        }
    }
    Ok((compiled, violations))
}

/// What one build of the crate's library showed.
pub(crate) enum Built {
    /// It built, and the compiler read these files (relative to the crate).
    Files(BTreeSet<PathBuf>),
    /// It failed, or showed nothing of what it read: why, with the
    /// compiler's first error.
    Refused(String),
}

/// Builds the crate's library for `target` and reads which of the crate's
/// files the compiler read, from its dependency-info file. A build that
/// does not list the crate's root among them shows nothing.
///
/// # Errors
///
/// Returns an error if cargo cannot be started or the crate's directory
/// cannot be resolved.
pub(crate) fn files_for(
    workspace: &Workspace,
    crate_dir: &Path,
    build_dir: &Path,
    target: &str,
) -> Result<Built, Error> {
    let crate_dir = fs::canonicalize(crate_dir).map_err(Error::path_io("resolve", crate_dir))?;
    let mut command = Command::new("cargo");
    command
        .current_dir(&workspace.root)
        .args(["check", "--locked", "--lib", "--quiet", "--package", CRATE])
        .args(["--message-format", "json-render-diagnostics", "--target"])
        .arg(target)
        .arg("--target-dir")
        .arg(build_dir);
    for name in CLEARED_ENVIRONMENT {
        command.env_remove(name);
    }
    let output = command.output().map_err(Error::io("cargo check"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let error = stderr
            .lines()
            .find(|line| line.contains(ALLOWLIST_ERROR))
            .map_or_else(|| first_error(&output.stderr), str::to_owned);
        return Ok(Built::Refused(format!("the crate did not build: {error}")));
    }
    let messages = String::from_utf8_lossy(&output.stdout);
    let built = messages
        .lines()
        .filter_map(|line| json::parse(line).ok())
        .find_map(|message| {
            matrix::built(&message, workspace, build_dir)
                .filter(|built| built.triple.name == CRATE && built.triple.target == target)
        });
    let read = built
        .and_then(|built| fs::read_to_string(&built.dep_info).ok())
        .map_or_else(Vec::new, |text| matrix::files_read(&text, &workspace.root));
    let mut files = BTreeSet::new();
    for file in &read {
        let inside = fs::canonicalize(file)
            .ok()
            .and_then(|file| file.strip_prefix(&crate_dir).ok().map(Path::to_path_buf))
            .filter(|file| file.extension().is_some_and(|extension| extension == "rs"));
        match inside {
            Some(file) => {
                files.insert(file);
            }
            None => {
                return Ok(Built::Refused(format!(
                    "the compiler read {}, which is not one of the crate's own .rs files, so \
                     this check did not scan it",
                    file.display()
                )));
            }
        }
    }
    if files.contains(&Path::new("src").join("lib.rs")) {
        Ok(Built::Files(files))
    } else {
        Ok(Built::Refused(
            "the compiler's dependency-info file does not list the crate's root, so nothing \
             shows which files it read"
                .to_owned(),
        ))
    }
}

/// Reads the copy and the ADR and returns them, or the copy's problems.
pub(crate) fn read_inventory(
    root: &Path,
    crate_dir: &Path,
) -> Result<Result<Inventory, Vec<Violation>>, Error> {
    let sites_path = crate_dir.join(SITES);
    let sites =
        fs::read_to_string(&sites_path).map_err(Error::path_io("read site list", &sites_path))?;
    let adr_path = root.join(inventory::ADR);
    let adr = fs::read_to_string(&adr_path).map_err(Error::path_io("read ADR", &adr_path))?;
    let inventory = match inventory::parse(&sites) {
        Ok(inventory) => inventory,
        Err(problem) => return Ok(Err(vec![Violation::Inventory(problem)])),
    };
    let problems = inventory::problems(&inventory, &adr);
    if problems.is_empty() {
        Ok(Ok(inventory))
    } else {
        Ok(Err(problems
            .into_iter()
            .map(Violation::Inventory)
            .collect()))
    }
}

/// The crate root's guard must accept exactly the allowlist among every
/// target the pinned compiler knows.
fn allowlist_violations(
    root: &Path,
    crate_dir: &Path,
) -> Result<(Vec<Violation>, Vec<String>), Error> {
    let lib = crate_dir.join("src").join("lib.rs");
    let text = fs::read_to_string(&lib).map_err(Error::path_io("read crate root", &lib))?;
    let accepted = match allowlist::accepted_targets(root, &text)? {
        Ok(accepted) => accepted,
        Err(error) => {
            return Ok((
                vec![Violation::Target {
                    target: "every target".to_owned(),
                    what: format!("the crate root's allowlist cannot be evaluated: {error}"),
                }],
                Vec::new(),
            ));
        }
    };
    let expected: BTreeSet<String> = ALLOWLIST
        .iter()
        .map(|target| (*target).to_owned())
        .collect();
    let allowed_cfgs: Vec<&allowlist::Cfg> = accepted
        .iter()
        .filter(|(target, _)| expected.contains(*target))
        .map(|(_, cfg)| cfg)
        .collect();
    let mut violations: Vec<Violation> = accepted
        .iter()
        .filter(|(target, cfg)| !expected.contains(*target) && !allowed_cfgs.contains(cfg))
        .map(|(target, _)| Violation::Target {
            target: target.clone(),
            what: "the crate root's guard accepts it, but it is not in ADR 0005's allowlist and \
                   its cfg set differs from every allowlisted target's"
                .to_owned(),
        })
        .collect();
    violations.extend(
        expected
            .iter()
            .filter(|target| !accepted.contains_key(*target))
            .map(|target| Violation::Target {
                target: target.clone(),
                what: "is in ADR 0005's allowlist, but the crate root's guard refuses it"
                    .to_owned(),
            }),
    );
    let equivalent = accepted
        .iter()
        .filter(|(target, cfg)| !expected.contains(*target) && allowed_cfgs.contains(cfg))
        .map(|(target, _)| target.clone())
        .collect();
    Ok((violations, equivalent))
}

/// Why naming a key that tells musl from glibc is refused.
pub(crate) const MUSL_KEY: &str = "names a cfg key whose value differs between the musl and glibc \
                                   targets (outside the crate root's allowlist), so a musl build \
                                   could compile other code than the glibc build whose status \
                                   it shares in sites.json";

/// Every use, in the crate's files, of a cfg key that tells musl from
/// glibc, other than `target_env` (the site reader's own rule) and the
/// crate root's allowlist.
pub(crate) fn musl_key_violations(
    crate_dir: &Path,
    keys: &BTreeSet<String>,
) -> Result<Vec<Violation>, Error> {
    let mut violations = Vec::new();
    let (files, _) = crate::pure::package_files(crate_dir, &crate_dir.join("target"))?;
    for file in files
        .iter()
        .filter(|file| file.extension().is_some_and(|extension| extension == "rs"))
    {
        let path = file.strip_prefix(crate_dir).unwrap_or(file).to_path_buf();
        let text = fs::read_to_string(file).map_err(Error::path_io("read source file", file))?;
        violations.extend(musl_key_uses(&path, &text, keys));
    }
    Ok(violations)
}

/// The uses of `keys` in one file's text.
pub(crate) fn musl_key_uses(path: &Path, text: &str, keys: &BTreeSet<String>) -> Vec<Violation> {
    let Ok(tokens) = rust_source::tokens_with_strings(text) else {
        return Vec::new();
    };
    let allowlist = if path == Path::new("src/lib.rs") {
        scan::allowlist_range(&tokens)
    } else {
        None
    };
    tokens
        .iter()
        .enumerate()
        .filter(|(index, token)| {
            rust_source::ident(Some(token))
                .is_some_and(|word| word != "target_env" && keys.contains(word))
                && !allowlist
                    .as_ref()
                    .is_some_and(|range| range.contains(index))
        })
        .map(|(_, token)| Violation::Rule {
            file: path.to_path_buf(),
            line: token.line,
            what: MUSL_KEY,
        })
        .collect()
}

/// The repository's build matrix (the targets in `rust-toolchain.toml` and
/// every workflow file) names none of the targets whose cfg set equals an
/// allowlisted one's: the crate's source cannot refuse them, so nothing
/// the repository builds or ships may target them.
fn matrix_violations(root: &Path, equivalent: &[String]) -> Result<Vec<Violation>, Error> {
    let mut sources = vec![root.join("rust-toolchain.toml")];
    let workflows = root.join(".github").join("workflows");
    if workflows.is_dir() {
        let (files, _) = crate::pure::package_files(&workflows, &workflows.join("none"))?;
        sources.extend(files);
    }
    let mut violations = Vec::new();
    for source in sources {
        let text = fs::read_to_string(&source).map_err(Error::path_io("read", &source))?;
        violations.extend(matrix_uses(&source, &text, equivalent));
    }
    Ok(violations)
}

/// The equivalent targets `text` names, as whole words.
pub(crate) fn matrix_uses(source: &Path, text: &str, equivalent: &[String]) -> Vec<Violation> {
    let words: BTreeSet<&str> = text
        .split(|next: char| !(next.is_ascii_alphanumeric() || next == '_' || next == '-'))
        .collect();
    equivalent
        .iter()
        .filter(|target| words.contains(target.as_str()))
        .map(|target| Violation::Target {
            target: target.clone(),
            what: format!(
                "named in {}: the crate's guard cannot refuse this target (its cfg set equals \
                 an allowlisted one's), so nothing in the repository may build for it",
                source.display()
            ),
        })
        .collect()
}

/// The crate's directory in the workspace.
pub(crate) fn crate_dir(workspace: &Workspace) -> Result<PathBuf, Error> {
    workspace
        .members
        .iter()
        .find(|member| member.name == CRATE)
        .map(|member| member.dir.clone())
        .ok_or(Error::UnexpectedOutput(
            "cargo metadata: ironafs-os is not a member",
        ))
}

/// Checks this repository, or with `self_test` proves on planted changes
/// to the crate's own files that each rule is caught.
///
/// # Errors
///
/// Returns the violations found, or why the check could not run.
pub(super) fn run(self_test: bool, output: &mut impl Write) -> Result<(), Error> {
    let workspace = metadata::load(None, &[])?;
    let crate_dir = crate_dir(&workspace)?;
    let build_dir = workspace.target_dir.join("xtask-check-os");
    let (compiled, mut violations) = compiled_files(&workspace, &crate_dir, &build_dir)?;
    let inventory = match read_inventory(&workspace.root, &crate_dir)? {
        Ok(inventory) => inventory,
        Err(problems) => {
            violations.extend(problems);
            return Err(rules(&violations));
        }
    };
    if self_test {
        if !violations.is_empty() {
            return Err(rules(&violations));
        }
        let adr_path = workspace.root.join(inventory::ADR);
        let adr = fs::read_to_string(&adr_path).map_err(Error::path_io("read ADR", &adr_path))?;
        return self_test::run(
            &workspace.root,
            &workspace.target_dir,
            &crate_dir,
            &adr,
            &inventory,
            &compiled,
            output,
        );
    }
    violations.extend(outside_violations(&workspace.root, &crate_dir)?);
    let (found, equivalent) = allowlist_violations(&workspace.root, &crate_dir)?;
    violations.extend(found);
    violations.extend(matrix_violations(&workspace.root, &equivalent)?);
    let keys = allowlist::musl_keys(&workspace.root)?;
    violations.extend(musl_key_violations(&crate_dir, &keys)?);
    let (files, unreadable) = crate_files(&crate_dir)?;
    violations.extend(unreadable);
    violations.extend(compare(&inventory, &files, &compiled));
    if !violations.is_empty() {
        return Err(rules(&violations));
    }
    let sites: usize = files.iter().map(|file| file.scan.sites.len()).sum();
    let implemented: Vec<&str> = inventory
        .rows
        .iter()
        .filter(|row| row.code.is_some())
        .map(|row| row.id.as_str())
        .collect();
    writeln!(
        output,
        "check-os: no unsafe outside {CRATE}; {} rows in sites.json match ADR 0005's table; \
         {sites} sites in {} files match the implemented rows ({}) on {}; the crate's guard \
         accepts exactly the allowlist among the compiler's targets, and also {}, whose cfg sets \
         equal an allowlisted target's",
        inventory.rows.len(),
        files.len(),
        implemented.join(", "),
        compiled.keys().cloned().collect::<Vec<_>>().join(", "),
        equivalent.join(", ")
    )
    .map_err(Error::io("report check-os"))
}

fn rules(violations: &[Violation]) -> Error {
    Error::Rules {
        check: CHECK,
        violations: violations.iter().map(ToString::to_string).collect(),
    }
}
