// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The verified install: the pinned packages fetched from the snapshot
//! through the signature-checked archive, checked against their pinned
//! hashes and versions, installed offline, and committed to a run-local
//! image.

use std::ffi::OsString;
use std::fmt;

use super::Failure;
use super::archive::prepare;
use super::definition::{Arch, Definition, Digest, PackagePin};
use super::runtime::{Run, Step, run_step};
use super::state::{Object, RunDir, State};

#[cfg(test)]
mod tests;

/// One way the installed bytes or versions differ from the pins.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum InstallViolation {
    HashMismatch {
        file: String,
        expected: Digest,
        found: Digest,
    },
    NotDownloaded {
        file: String,
    },
    Unexpected {
        file: String,
    },
    VersionMismatch {
        package: String,
        expected: String,
        found: String,
    },
    NotInstalled {
        package: String,
    },
    Unpinned {
        package: String,
        version: String,
    },
}

impl fmt::Display for InstallViolation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HashMismatch {
                file,
                expected,
                found,
            } => write!(
                formatter,
                "{file} has SHA-256 {found}, but {expected} is pinned"
            ),
            Self::NotDownloaded { file } => write!(formatter, "{file} was not downloaded"),
            Self::Unexpected { file } => {
                write!(formatter, "{file} was downloaded but is not pinned")
            }
            Self::VersionMismatch {
                package,
                expected,
                found,
            } => write!(
                formatter,
                "{package} {found} is installed, but {expected} is pinned"
            ),
            Self::NotInstalled { package } => write!(formatter, "{package} is not installed"),
            Self::Unpinned { package, version } => {
                write!(
                    formatter,
                    "{package} {version} was installed but is not pinned"
                )
            }
        }
    }
}

/// Downloads the packages named as `name=version` arguments.
const DOWNLOAD: &str = "set -eu
mkdir -p /var/cache/cell
chown _apt /var/cache/cell
cd /var/cache/cell
apt-get download \"$@\"
";

/// Hashes every downloaded package.
const HASH: &str = "set -eu
sha256sum /var/cache/cell/*.deb
";

/// Removes the apt source and lists, so nothing after the hash check can
/// fetch.
const REMOVE_SOURCES: &str = "set -eu
rm -f /etc/apt/sources.list.d/cell.sources
rm -rf /var/lib/apt/lists/*
";

/// Installs the checked packages offline: a package missing from the
/// closure fails here.
const INSTALL: &str = "set -eu
dpkg -i /var/cache/cell/*.deb
rm -rf /var/cache/cell
";

/// The `dpkg-query` format: one `<package> <version>` line per package.
const QUERY_FORMAT: &str = "${Package} ${Version}\\n";

/// Reads `sha256sum` output as file names, without their directory, and
/// digests.
///
/// # Errors
///
/// Returns [`Failure::UnexpectedOutput`] for a line that is not a digest
/// and a path.
pub(crate) fn parse_sha256sum(text: &str) -> Result<Vec<(String, Digest)>, Failure> {
    text.lines()
        .enumerate()
        .map(|(index, line)| {
            line.split_once("  ")
                .and_then(|(digest, path)| {
                    let file = path.rsplit('/').next().filter(|file| !file.is_empty())?;
                    Some((file.to_owned(), Digest::parse_hex(digest)?))
                })
                .ok_or(Failure::UnexpectedOutput {
                    step: "sha256sum",
                    line: index + 1,
                })
        })
        .collect()
}

/// Reads `dpkg-query` output in [`QUERY_FORMAT`].
///
/// # Errors
///
/// Returns [`Failure::UnexpectedOutput`] for a line that is not a package
/// and a version.
pub(crate) fn parse_dpkg_query(text: &str) -> Result<Vec<(String, String)>, Failure> {
    text.lines()
        .enumerate()
        .map(|(index, line)| {
            line.split_once(' ')
                .filter(|(package, version)| {
                    !package.is_empty() && !version.is_empty() && !version.contains(' ')
                })
                .map(|(package, version)| (package.to_owned(), version.to_owned()))
                .ok_or(Failure::UnexpectedOutput {
                    step: "dpkg-query",
                    line: index + 1,
                })
        })
        .collect()
}

/// Compares the downloaded files' digests with the pins: every pinned
/// file must be present with its pinned digest, and nothing else may be.
pub(crate) fn compare_hashes(
    pins: &[&PackagePin],
    measured: &[(String, Digest)],
) -> Vec<InstallViolation> {
    let mut violations = Vec::new();
    for pin in pins {
        let file = pin.local_file();
        match measured.iter().find(|(name, _)| *name == file) {
            None => violations.push(InstallViolation::NotDownloaded { file }),
            Some((_, found)) if *found != pin.sha256 => {
                violations.push(InstallViolation::HashMismatch {
                    file,
                    expected: pin.sha256,
                    found: *found,
                });
            }
            Some(_) => {}
        }
    }
    for (file, _) in measured {
        if !pins.iter().any(|pin| pin.local_file() == *file) {
            violations.push(InstallViolation::Unexpected { file: file.clone() });
        }
    }
    violations
}

/// Compares the packages installed with the pins: every pin is installed
/// at its version, and every package new or changed since `base` is
/// pinned.
pub(crate) fn compare_installed(
    pins: &[&PackagePin],
    base: &[(String, String)],
    after: &[(String, String)],
) -> Vec<InstallViolation> {
    let mut violations = Vec::new();
    for pin in pins {
        match after.iter().find(|(package, _)| *package == pin.name) {
            None => violations.push(InstallViolation::NotInstalled {
                package: pin.name.clone(),
            }),
            Some((_, found)) if *found != pin.version => {
                violations.push(InstallViolation::VersionMismatch {
                    package: pin.name.clone(),
                    expected: pin.version.clone(),
                    found: found.clone(),
                });
            }
            Some(_) => {}
        }
    }
    for (package, version) in after {
        let unchanged = base
            .iter()
            .any(|entry| entry.0 == *package && entry.1 == *version);
        if !unchanged && !pins.iter().any(|pin| pin.name == *package) {
            violations.push(InstallViolation::Unpinned {
                package: package.clone(),
                version: version.clone(),
            });
        }
    }
    violations
}

/// Installs `arch`'s pins into a builder container and commits it as the
/// image `ironafs-cell:<run id>`, which it returns.
///
/// The packages are downloaded through apt, which checks them against the
/// snapshot's signed index; then their SHA-256 digests, taken from the
/// bytes `dpkg` will install, are compared with the pins. Only then are the
/// sources removed and the packages installed offline. The installed
/// versions are compared with the pins before the image is committed. Every
/// container and image is recorded in `state` before it is created.
///
/// # Errors
///
/// Returns [`Failure::Install`] listing every hash or version violation,
/// and the first failed step or check otherwise.
pub(crate) fn install(
    run: &mut impl Run,
    def: &Definition,
    arch: Arch,
    dir: &RunDir,
    state: &mut State,
) -> Result<String, Failure> {
    let container = format!("ironafs-cell-{}-builder", dir.id);
    let pins: Vec<&PackagePin> = def.packages.iter().filter(|pin| pin.arch == arch).collect();
    prepare(run, def, arch, dir, state, &container)?;
    let query = [
        "exec",
        container.as_str(),
        "dpkg-query",
        "-W",
        "-f",
        QUERY_FORMAT,
    ];
    let base = parse_dpkg_query(&run_step(run, &Step::new("list base packages", query))?)?;

    let mut download = Step::new(
        "apt-get download",
        ["exec", &container, "sh", "-c", DOWNLOAD, "sh"],
    );
    download.args.extend(
        pins.iter()
            .map(|pin| OsString::from(format!("{}={}", pin.name, pin.version))),
    );
    run_step(run, &download)?;
    let measured = parse_sha256sum(&run_step(
        run,
        &Step::new("sha256sum", ["exec", &container, "sh", "-c", HASH]),
    )?)?;
    let violations = compare_hashes(&pins, &measured);
    if !violations.is_empty() {
        return Err(Failure::Install(violations));
    }

    run_step(
        run,
        &Step::new(
            "remove sources",
            ["exec", &container, "sh", "-c", REMOVE_SOURCES],
        ),
    )?;
    run_step(
        run,
        &Step::new("dpkg -i", ["exec", &container, "sh", "-c", INSTALL]),
    )?;
    let after = parse_dpkg_query(&run_step(
        run,
        &Step::new("list installed packages", query),
    )?)?;
    let violations = compare_installed(&pins, &base, &after);
    if !violations.is_empty() {
        return Err(Failure::Install(violations));
    }

    let image = format!("ironafs-cell:{}", dir.id);
    state.record(dir, Object::Image(image.clone()))?;
    run_step(
        run,
        &Step::new("commit image", ["commit", &container, &image]),
    )?;
    Ok(image)
}
