// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The pinned archive as the only apt source of a builder container: the
//! sources text is generated, never configured, and apt's update must
//! verify the snapshot's signature against the image's archive keyring.

use super::Failure;
use super::definition::{Arch, Definition, DefinitionError};
use super::runtime::{Ran, Run, Step, require_success, run_step};
use super::state::{Object, RunDir, State};

#[cfg(test)]
mod tests;

/// The archive keyring the pinned image ships, which apt checks the
/// snapshot's signed `InRelease` against.
const KEYRING: &str = "/usr/share/keyrings/debian-archive-keyring.gpg";

/// Writes the apt source from standard input, removes the image's own
/// sources, and stops packages from starting services while they install.
const WRITE_SOURCES: &str = "set -eu
cat > /etc/apt/sources.list.d/cell.sources
rm -f /etc/apt/sources.list.d/debian.sources /etc/apt/sources.list
printf '#!/bin/sh\\nexit 101\\n' > /usr/sbin/policy-rc.d
chmod 0755 /usr/sbin/policy-rc.d
";

/// apt output that means a source was not verified, or was accepted with a
/// weaker check. Any warning is refused.
const UNVERIFIED: &[&str] = &[
    "NO_PUBKEY",
    "EXPKEYSIG",
    "BADSIG",
    "is not signed",
    "GPG error",
    "W: ",
];

/// The name of the archive update step.
const UPDATE: &str = "apt-get update";

/// The single deb822 source for the pinned snapshot, checked against the
/// image's archive keyring. It carries no option that weakens the check.
pub(crate) fn sources(def: &Definition) -> String {
    format!(
        "Types: deb\nURIs: {}\nSuites: {}\nComponents: {}\nSigned-By: {KEYRING}\n",
        def.snapshot.url(),
        def.snapshot.suite,
        def.snapshot.component
    )
}

/// Refuses an archive update that failed, or whose output reports an
/// unverified source or any warning, even with a zero exit status.
///
/// # Errors
///
/// Returns [`Failure::Step`] for a failed update and
/// [`Failure::ArchiveNotVerified`] naming the first such line.
pub(crate) fn check_apt_update(ran: &Ran) -> Result<(), Failure> {
    require_success(&Step::new(UPDATE, []), ran)?;
    let stdout = String::from_utf8_lossy(&ran.stdout);
    let stderr = String::from_utf8_lossy(&ran.stderr);
    match stdout
        .lines()
        .chain(stderr.lines())
        .find(|line| UNVERIFIED.iter().any(|marker| line.contains(marker)))
    {
        Some(line) => Err(Failure::ArchiveNotVerified(line.to_owned())),
        None => Ok(()),
    }
}

/// Starts a builder container from the pinned platform image, with the
/// snapshot as its only apt source, and updates and checks the archive.
/// The container, named `container`, is recorded before it starts.
///
/// # Errors
///
/// Returns [`Failure::Definition`] if `arch` has no platform pin, before
/// any step runs, and the first failed step or check otherwise.
pub(crate) fn prepare(
    run: &mut impl Run,
    def: &Definition,
    arch: Arch,
    dir: &RunDir,
    state: &mut State,
    container: &str,
) -> Result<(), Failure> {
    let digest = def
        .image
        .platforms
        .iter()
        .find(|(pinned, _)| *pinned == arch)
        .map(|(_, digest)| *digest)
        .ok_or_else(|| {
            Failure::Definition(vec![DefinitionError::MissingArch {
                arch: arch.debian(),
            }])
        })?;
    let image = format!("{}@sha256:{digest}", def.image.repository);
    let platform = format!("linux/{}", arch.debian());
    run_step(
        run,
        &Step::new("pull image", ["pull", "--platform", &platform, &image]),
    )?;
    state.record(dir, Object::Container(container.to_owned()))?;
    run_step(
        run,
        &Step::new(
            "start builder",
            [
                "run",
                "-d",
                "--name",
                container,
                "--platform",
                &platform,
                "-e",
                "DEBIAN_FRONTEND=noninteractive",
                &image,
                "sleep",
                "infinity",
            ],
        ),
    )?;
    run_step(
        run,
        &Step {
            stdin: Some(sources(def).into_bytes()),
            ..Step::new(
                "write sources",
                ["exec", "-i", container, "sh", "-c", WRITE_SOURCES],
            )
        },
    )?;
    let ran = run.run(&Step::new(UPDATE, ["exec", container, "apt-get", "update"]))?;
    check_apt_update(&ran)
}
