// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! `cargo xtask cell pin`: resolves the required packages' closure in the
//! pinned image against the pinned snapshot, and writes it into the pin
//! file as exact versions and hashes.

use std::ffi::OsString;

use super::Failure;
use super::archive::prepare;
use super::definition::{Arch, Definition, Digest, PackagePin, REQUIRED};
use super::runtime::{Run, Step, run_step};
use super::state::{RunDir, State};

#[cfg(test)]
mod tests;

/// One package apt would fetch.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Printed {
    /// The file's path in the archive's pool.
    pub(crate) pool: String,
    pub(crate) name: String,
    pub(crate) version: String,
    pub(crate) sha256: Digest,
}

/// The step that prints the closure's download locations.
const PRINT_URIS: &str = "apt-get print uris";

/// Reads `apt-get --print-uris` output: one line per package,
/// `'<url>' <file> <size> SHA256:<hex>`. The URL must lie in the snapshot's
/// pool under `base_url`; the URL and the file name are percent-decoded,
/// and the file name gives the package's name and version
/// (`<name>_<version>_<arch>.deb`).
///
/// # Errors
///
/// Returns [`Failure::UnexpectedOutput`] for any other line.
pub(crate) fn parse_print_uris(text: &str, base_url: &str) -> Result<Vec<Printed>, Failure> {
    text.lines()
        .enumerate()
        .map(|(index, line)| {
            printed(line, base_url).ok_or(Failure::UnexpectedOutput {
                step: PRINT_URIS,
                line: index + 1,
            })
        })
        .collect()
}

fn printed(line: &str, base_url: &str) -> Option<Printed> {
    let (url, rest) = line.strip_prefix('\'')?.split_once("' ")?;
    let pool = percent_decode(url.strip_prefix(base_url)?.strip_prefix('/')?)?;
    if !pool.starts_with("pool/") {
        return None;
    }
    let mut fields = rest.split(' ');
    let (Some(file), Some(size), Some(hash), None) =
        (fields.next(), fields.next(), fields.next(), fields.next())
    else {
        return None;
    };
    size.parse::<u64>().ok()?;
    let sha256 = Digest::parse_hex(hash.strip_prefix("SHA256:")?)?;
    let file = percent_decode(file)?;
    let mut parts = file.strip_suffix(".deb")?.split('_');
    let (Some(name), Some(version), Some(_), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return None;
    };
    Some(Printed {
        pool,
        name: name.to_owned(),
        version: version.to_owned(),
        sha256,
    })
}

/// Decodes `%XX` escapes; `None` for a malformed escape or a result that
/// is not UTF-8.
fn percent_decode(text: &str) -> Option<String> {
    let mut bytes = Vec::with_capacity(text.len());
    let mut rest = text.as_bytes();
    while let Some((first, tail)) = rest.split_first() {
        if *first == b'%' {
            let (escape, tail) = tail.split_at_checked(2)?;
            let escape = std::str::from_utf8(escape).ok()?;
            bytes.push(u8::from_str_radix(escape, 16).ok()?);
            rest = tail;
        } else {
            bytes.push(*first);
            rest = tail;
        }
    }
    String::from_utf8(bytes).ok()
}

/// Resolves, for each architecture, the closure apt would install for the
/// [`REQUIRED`] packages at their pinned versions, in a builder container
/// started from that platform's pinned image with the snapshot as its only,
/// signature-checked source. Returns the pins sorted by architecture and
/// name. The builders are recorded in `state`; the caller tears them down.
///
/// A foreign architecture's container runs under the engine's emulation.
///
/// # Errors
///
/// Returns the first failed step or check.
pub(crate) fn pin(
    run: &mut impl Run,
    def: &Definition,
    dir: &RunDir,
    state: &mut State,
) -> Result<Vec<PackagePin>, Failure> {
    let base_url = def.snapshot.url();
    let mut packages = Vec::new();
    for arch in Arch::ALL {
        let container = format!("ironafs-cell-{}-pin-{}", dir.id, arch.debian());
        prepare(run, def, arch, dir, state, &container)?;
        let mut step = Step::new(
            PRINT_URIS,
            [
                "exec",
                &container,
                "apt-get",
                "install",
                "-y",
                "--no-install-recommends",
                "--print-uris",
                "-qq",
            ],
        );
        step.args.extend(
            REQUIRED
                .iter()
                .map(|(name, version)| OsString::from(format!("{name}={version}"))),
        );
        let printed = parse_print_uris(&run_step(run, &step)?, &base_url)?;
        packages.extend(printed.into_iter().map(|printed| PackagePin {
            arch,
            name: printed.name,
            version: printed.version,
            sha256: printed.sha256,
            filename: printed.pool,
        }));
    }
    packages.sort_by(|left, right| (left.arch, &left.name).cmp(&(right.arch, &right.name)));
    Ok(packages)
}

/// Writes `packages` into the pin file `text`: every `package` line is
/// replaced by the new ones, sorted, at the place of the first, or at the
/// end if there was none. Every other line is kept as it was. The same
/// inputs give the same bytes.
pub(crate) fn render(text: &str, packages: &[PackagePin]) -> String {
    let mut sorted: Vec<&PackagePin> = packages.iter().collect();
    sorted.sort_by(|left, right| (left.arch, &left.name).cmp(&(right.arch, &right.name)));
    let lines = sorted.iter().fold(String::new(), |text, pin| {
        text + &format!(
            "package {} {} {} {} {}\n",
            pin.arch.debian(),
            pin.name,
            pin.version,
            pin.sha256,
            pin.filename
        )
    });
    let mut rendered = String::new();
    let mut placed = false;
    for line in text.lines() {
        if line.split_ascii_whitespace().next() == Some("package") {
            if !placed {
                rendered.push_str(&lines);
                placed = true;
            }
        } else {
            rendered.push_str(line);
            rendered.push('\n');
        }
    }
    if !placed {
        rendered.push_str(&lines);
    }
    rendered
}
