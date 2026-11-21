// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The cell's pinned definition: the image by digest, the dated package
//! snapshot, the exact package builds and the synthetic names, read from
//! `crates/xtask/cell/pins.txt` and refused unless every pin is exact.

use std::fmt;
use std::path::PathBuf;

use super::Failure;

#[cfg(test)]
mod tests;

/// The packages the cell needs, at the versions ADR 0004 L40 pins. Each is
/// required for every architecture in [`Arch::ALL`].
pub(crate) const REQUIRED: &[(&str, &str)] = &[
    ("openafs-fileserver", "1.8.13.2-1"),
    ("openafs-dbserver", "1.8.13.2-1"),
    ("openafs-client", "1.8.13.2-1"),
    ("openafs-krb5", "1.8.13.2-1"),
    ("krb5-kdc", "1.21.3-5+deb13u1"),
    ("krb5-admin-server", "1.21.3-5+deb13u1"),
    ("krb5-user", "1.21.3-5+deb13u1"),
];

/// The whole pinned definition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Definition {
    pub(crate) image: ImagePin,
    pub(crate) snapshot: Snapshot,
    pub(crate) packages: Vec<PackagePin>,
    pub(crate) cell: CellNames,
}

/// The base image: its repository and tag, the digest of its multi-platform
/// index, and the digest of each platform's manifest.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ImagePin {
    pub(crate) repository: String,
    pub(crate) tag: String,
    pub(crate) index: Digest,
    pub(crate) platforms: Vec<(Arch, Digest)>,
}

/// A host architecture the cell has pins for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Arch {
    Amd64,
    Arm64,
}

/// A SHA-256 digest.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Digest(pub(crate) [u8; 32]);

/// The dated snapshot archive the packages come from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Snapshot {
    pub(crate) base: String,
    pub(crate) timestamp: String,
    pub(crate) suite: String,
    pub(crate) component: String,
}

/// One package build, pinned for one host architecture.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PackagePin {
    pub(crate) arch: Arch,
    pub(crate) name: String,
    pub(crate) version: String,
    pub(crate) sha256: Digest,
    /// The file's path in the archive's pool, as its index lists it.
    pub(crate) filename: String,
}

/// The cell's synthetic names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CellNames {
    pub(crate) cell: String,
    pub(crate) realm: String,
}

/// One refusal of the pin file. Lines are numbered from 1.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum DefinitionError {
    UnknownKey {
        line: usize,
    },
    Malformed {
        line: usize,
    },
    Duplicate {
        line: usize,
        what: String,
    },
    BadDigest {
        line: usize,
    },
    FloatingVersion {
        line: usize,
        name: String,
    },
    CredentialsInUrl {
        line: usize,
    },
    InsecureOption {
        line: usize,
    },
    Missing(&'static str),
    MissingArch {
        arch: &'static str,
    },
    RequiredPinWrong {
        arch: &'static str,
        name: &'static str,
        expected: &'static str,
        found: Option<String>,
    },
    NotSynthetic {
        line: usize,
    },
}

impl fmt::Display for DefinitionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownKey { line } => write!(formatter, "line {line}: unknown entry"),
            Self::Malformed { line } => write!(formatter, "line {line}: malformed entry"),
            Self::Duplicate { line, what } => {
                write!(formatter, "line {line}: {what} is pinned twice")
            }
            Self::BadDigest { line } => write!(
                formatter,
                "line {line}: a digest is not 64 lowercase hexadecimal digits"
            ),
            Self::FloatingVersion { line, name } => {
                write!(formatter, "line {line}: the version of {name} is not exact")
            }
            Self::CredentialsInUrl { line } => write!(
                formatter,
                "line {line}: a pinned location carries credentials or a query"
            ),
            Self::InsecureOption { line } => write!(
                formatter,
                "line {line}: an option that weakens the archive's signature check"
            ),
            Self::Missing(key) => write!(formatter, "no {key} entry"),
            Self::MissingArch { arch } => write!(formatter, "no pins for {arch}"),
            Self::RequiredPinWrong {
                arch,
                name,
                expected,
                found,
            } => match found {
                Some(found) => write!(
                    formatter,
                    "{name} for {arch} is pinned at {found}, not {expected}"
                ),
                None => write!(formatter, "{name} {expected} for {arch} is not pinned"),
            },
            Self::NotSynthetic { line } => write!(
                formatter,
                "line {line}: the cell and realm names must end .invalid and .INVALID"
            ),
        }
    }
}

impl Arch {
    /// Every architecture the cell must have pins for.
    pub(crate) const ALL: [Self; 2] = [Self::Amd64, Self::Arm64];

    /// The architecture this xtask was built for, if the cell has pins
    /// for it.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "TODO(#47): `cell up` installs for the host")
    )]
    pub(crate) fn host() -> Option<Self> {
        match std::env::consts::ARCH {
            "x86_64" => Some(Self::Amd64),
            "aarch64" => Some(Self::Arm64),
            _ => None,
        }
    }

    /// Debian's name for the architecture.
    pub(crate) const fn debian(self) -> &'static str {
        match self {
            Self::Amd64 => "amd64",
            Self::Arm64 => "arm64",
        }
    }

    fn from_debian(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|arch| arch.debian() == name)
    }
}

impl Digest {
    /// Reads exactly 64 lowercase hexadecimal digits.
    pub(crate) fn parse_hex(text: &str) -> Option<Self> {
        let digits = text.as_bytes();
        if digits.len() != 64 {
            return None;
        }
        let mut bytes = [0; 32];
        let (pairs, _) = digits.as_chunks::<2>();
        for (byte, [high, low]) in bytes.iter_mut().zip(pairs) {
            *byte = (hex_digit(*high)? << 4) | hex_digit(*low)?;
        }
        Some(Self(bytes))
    }
}

fn hex_digit(digit: u8) -> Option<u8> {
    match digit {
        b'0'..=b'9' => Some(digit - b'0'),
        b'a'..=b'f' => Some(digit - b'a' + 10),
        _ => None,
    }
}

impl fmt::Display for Digest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0
            .iter()
            .try_for_each(|byte| write!(formatter, "{byte:02x}"))
    }
}

impl fmt::Debug for Digest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Digest({self})")
    }
}

impl Snapshot {
    /// The archive's root at the pinned time.
    pub(crate) fn url(&self) -> String {
        format!("{}/{}", self.base, self.timestamp)
    }
}

impl PackagePin {
    /// The name apt gives the downloaded file.
    ///
    /// apt names it `<name>_<version>_<architecture>.deb`, with the
    /// version's epoch colon written `%3a`. The architecture is the
    /// package's own, `all` for an architecture-independent package, which
    /// the pool file name carries.
    pub(crate) fn local_file(&self) -> String {
        let deb_arch = self
            .filename
            .rsplit_once('_')
            .and_then(|(_, tail)| tail.strip_suffix(".deb"))
            .unwrap_or_else(|| self.arch.debian());
        format!(
            "{}_{}_{deb_arch}.deb",
            self.name,
            self.version.replace(':', "%3a")
        )
    }
}

/// Parses a pin file, reporting every error rather than the first.
///
/// # Errors
///
/// Returns every [`DefinitionError`] found.
pub(crate) fn parse(text: &str) -> Result<Definition, Vec<DefinitionError>> {
    let mut parser = Parser::default();
    for (index, line) in text.lines().enumerate() {
        let number = index + 1;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Err(error) = parser.line(number, trimmed) {
            parser.errors.push(error);
        }
    }
    parser.finish()
}

/// Fields apt reads as options that weaken or bypass its signature check.
/// A field starting `[` is an apt option list of any kind, refused
/// whatever it holds: the sources text is generated, never configured.
const INSECURE: &[&str] = &["trusted=yes", "allow-insecure=yes", "allow-weak=yes"];

/// Characters that make a version a pattern rather than one build.
const FLOATING: &[char] = &['*', '?', '[', '>', '<', '=', ' '];

#[derive(Default)]
struct Parser {
    errors: Vec<DefinitionError>,
    /// The entries seen, valid or not, so a refused line is not reported a
    /// second time as missing.
    seen: Vec<&'static str>,
    image: Option<(String, String, Digest)>,
    platforms: Vec<(Arch, Digest)>,
    /// Architectures named by an `image-platform` line, valid or not.
    platform_lines: Vec<Arch>,
    snapshot: Option<Snapshot>,
    cell: Option<CellNames>,
    packages: Vec<PackagePin>,
    /// Packages named by a refused line, so the required check does not
    /// report them again.
    refused: Vec<(Arch, String)>,
}

impl Parser {
    fn line(&mut self, line: usize, text: &str) -> Result<(), DefinitionError> {
        let mut fields: Vec<&str> = text.split_ascii_whitespace().collect();
        let before = fields.len();
        fields.retain(|field| !INSECURE.contains(field) && !field.starts_with('['));
        let insecure = fields.len() != before;
        let result = match fields.as_slice() {
            ["image", reference] => self.image(line, reference),
            ["image-platform", arch, digest] => self.platform(line, arch, digest),
            ["snapshot", base, timestamp, suite, component] => {
                self.snapshot(line, base, timestamp, suite, component)
            }
            ["cell", cell, realm] => self.cell(line, cell, realm),
            ["package", arch, name, version, digest, filename] => {
                self.package(line, arch, name, version, digest, filename)
            }
            [key, ..] => {
                if let Some(known) = ["image", "image-platform", "snapshot", "cell", "package"]
                    .into_iter()
                    .find(|known| known == key)
                {
                    self.seen.push(known);
                    Err(DefinitionError::Malformed { line })
                } else {
                    Err(DefinitionError::UnknownKey { line })
                }
            }
            [] => Err(DefinitionError::Malformed { line }),
        };
        if insecure {
            return Err(DefinitionError::InsecureOption { line });
        }
        result
    }

    fn image(&mut self, line: usize, reference: &str) -> Result<(), DefinitionError> {
        self.seen.push("image");
        let (name, digest) = reference
            .split_once("@sha256:")
            .ok_or(DefinitionError::Malformed { line })?;
        if name.contains('@') {
            return Err(DefinitionError::CredentialsInUrl { line });
        }
        let index = Digest::parse_hex(digest).ok_or(DefinitionError::BadDigest { line })?;
        let (repository, tag) = name
            .rsplit_once(':')
            .filter(|(repository, tag)| {
                is_word(repository, |c| {
                    c.is_ascii_lowercase() || c.is_ascii_digit() || "._/-".contains(c)
                }) && is_word(tag, |c| c.is_ascii_alphanumeric() || "._-".contains(c))
            })
            .ok_or(DefinitionError::Malformed { line })?;
        if self.image.is_some() {
            return Err(DefinitionError::Duplicate {
                line,
                what: "image".to_owned(),
            });
        }
        self.image = Some((repository.to_owned(), tag.to_owned(), index));
        Ok(())
    }

    fn platform(&mut self, line: usize, arch: &str, digest: &str) -> Result<(), DefinitionError> {
        let arch = Arch::from_debian(arch).ok_or(DefinitionError::Malformed { line })?;
        let duplicate = self.platform_lines.contains(&arch);
        self.platform_lines.push(arch);
        let digest = digest
            .strip_prefix("sha256:")
            .ok_or(DefinitionError::Malformed { line })?;
        let digest = Digest::parse_hex(digest).ok_or(DefinitionError::BadDigest { line })?;
        if duplicate {
            return Err(DefinitionError::Duplicate {
                line,
                what: format!("image-platform {}", arch.debian()),
            });
        }
        self.platforms.push((arch, digest));
        Ok(())
    }

    fn snapshot(
        &mut self,
        line: usize,
        base: &str,
        timestamp: &str,
        suite: &str,
        component: &str,
    ) -> Result<(), DefinitionError> {
        self.seen.push("snapshot");
        let authority = base
            .split_once("://")
            .map_or(base, |(_, rest)| rest.split('/').next().unwrap_or(rest));
        if authority.contains('@') || base.contains('?') {
            return Err(DefinitionError::CredentialsInUrl { line });
        }
        let path_ok = base.strip_prefix("https://").is_some_and(|rest| {
            !authority.is_empty()
                && !rest.ends_with('/')
                && is_word(rest, |c| c.is_ascii_alphanumeric() || "./-_~".contains(c))
        });
        let simple = |word: &str| {
            is_word(word, |c| {
                c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'
            })
        };
        if !path_ok || !is_timestamp(timestamp) || !simple(suite) || !simple(component) {
            return Err(DefinitionError::Malformed { line });
        }
        if self.snapshot.is_some() {
            return Err(DefinitionError::Duplicate {
                line,
                what: "snapshot".to_owned(),
            });
        }
        self.snapshot = Some(Snapshot {
            base: base.to_owned(),
            timestamp: timestamp.to_owned(),
            suite: suite.to_owned(),
            component: component.to_owned(),
        });
        Ok(())
    }

    fn cell(&mut self, line: usize, cell: &str, realm: &str) -> Result<(), DefinitionError> {
        self.seen.push("cell");
        let cell_ok = is_word(cell, |c| {
            c.is_ascii_lowercase() || c.is_ascii_digit() || ".-".contains(c)
        });
        let realm_ok = is_word(realm, |c| {
            c.is_ascii_uppercase() || c.is_ascii_digit() || ".-".contains(c)
        });
        if !cell_ok || !realm_ok {
            return Err(DefinitionError::Malformed { line });
        }
        if !cell.ends_with(".invalid") || !realm.ends_with(".INVALID") {
            return Err(DefinitionError::NotSynthetic { line });
        }
        if self.cell.is_some() {
            return Err(DefinitionError::Duplicate {
                line,
                what: "cell".to_owned(),
            });
        }
        self.cell = Some(CellNames {
            cell: cell.to_owned(),
            realm: realm.to_owned(),
        });
        Ok(())
    }

    fn package(
        &mut self,
        line: usize,
        arch: &str,
        name: &str,
        version: &str,
        digest: &str,
        filename: &str,
    ) -> Result<(), DefinitionError> {
        self.seen.push("package");
        let arches: Vec<Arch> = if arch == "all" {
            Arch::ALL.to_vec()
        } else {
            vec![Arch::from_debian(arch).ok_or(DefinitionError::Malformed { line })?]
        };
        let name_ok = name.len() >= 2
            && name.starts_with(|c: char| c.is_ascii_lowercase() || c.is_ascii_digit())
            && is_word(name, |c| {
                c.is_ascii_lowercase() || c.is_ascii_digit() || "+.-".contains(c)
            });
        if !name_ok {
            return Err(DefinitionError::Malformed { line });
        }
        let result = Self::package_fields(line, &arches, name, version, digest, filename);
        let sha256 = match result {
            Ok(sha256) => sha256,
            Err(error) => {
                self.refused
                    .extend(arches.iter().map(|arch| (*arch, name.to_owned())));
                return Err(error);
            }
        };
        for arch in &arches {
            if self
                .packages
                .iter()
                .any(|pin| pin.arch == *arch && pin.name == name)
            {
                return Err(DefinitionError::Duplicate {
                    line,
                    what: format!("package {} {name}", arch.debian()),
                });
            }
        }
        self.packages
            .extend(arches.into_iter().map(|arch| PackagePin {
                arch,
                name: name.to_owned(),
                version: version.to_owned(),
                sha256,
                filename: filename.to_owned(),
            }));
        Ok(())
    }

    /// Checks a package line's version, digest and pool file name.
    fn package_fields(
        line: usize,
        arches: &[Arch],
        name: &str,
        version: &str,
        digest: &str,
        filename: &str,
    ) -> Result<Digest, DefinitionError> {
        if version.is_empty() || version.contains(FLOATING) {
            return Err(DefinitionError::FloatingVersion {
                line,
                name: name.to_owned(),
            });
        }
        let version_ok = version.starts_with(|c: char| c.is_ascii_digit())
            && is_word(version, |c| {
                c.is_ascii_alphanumeric() || ".+~:-".contains(c)
            });
        if !version_ok {
            return Err(DefinitionError::Malformed { line });
        }
        let sha256 = Digest::parse_hex(digest).ok_or(DefinitionError::BadDigest { line })?;
        // The pool file name carries the version without its epoch.
        let upstream = version.split_once(':').map_or(version, |(_, rest)| rest);
        let file_ok = filename.strip_prefix("pool/").is_some_and(|path| {
            is_word(path, |c| c.is_ascii_alphanumeric() || "/._+~-".contains(c))
                && !path
                    .split('/')
                    .any(|part| part.is_empty() || part == "." || part == "..")
        }) && filename
            .rsplit('/')
            .next()
            .and_then(|file| file.strip_suffix(".deb"))
            .is_some_and(|stem| {
                let mut parts = stem.split('_');
                let (Some(stem_name), Some(stem_version), Some(stem_arch), None) =
                    (parts.next(), parts.next(), parts.next(), parts.next())
                else {
                    return false;
                };
                stem_name == name
                    && stem_version == upstream
                    && arches
                        .iter()
                        .all(|arch| stem_arch == "all" || stem_arch == arch.debian())
            });
        if !file_ok {
            return Err(DefinitionError::Malformed { line });
        }
        Ok(sha256)
    }

    fn finish(mut self) -> Result<Definition, Vec<DefinitionError>> {
        for key in ["image", "snapshot", "cell"] {
            if !self.seen.contains(&key) {
                self.errors.push(DefinitionError::Missing(key));
            }
        }
        for arch in Arch::ALL {
            if !self.platform_lines.contains(&arch) {
                self.errors.push(DefinitionError::MissingArch {
                    arch: arch.debian(),
                });
                continue;
            }
            for (name, expected) in REQUIRED {
                if self
                    .refused
                    .iter()
                    .any(|(refused, refused_name)| *refused == arch && refused_name == name)
                {
                    continue;
                }
                let found = self
                    .packages
                    .iter()
                    .find(|pin| pin.arch == arch && pin.name == *name)
                    .map(|pin| pin.version.clone());
                if found.as_deref() != Some(*expected) {
                    self.errors.push(DefinitionError::RequiredPinWrong {
                        arch: arch.debian(),
                        name,
                        expected,
                        found,
                    });
                }
            }
        }
        match (self.image, self.snapshot, self.cell) {
            (Some((repository, tag, index)), Some(snapshot), Some(cell))
                if self.errors.is_empty() =>
            {
                Ok(Definition {
                    image: ImagePin {
                        repository,
                        tag,
                        index,
                        platforms: self.platforms,
                    },
                    snapshot,
                    packages: self.packages,
                    cell,
                })
            }
            _ => Err(self.errors),
        }
    }
}

/// Whether `word` is non-empty and every character passes `allowed`.
fn is_word(word: &str, allowed: impl Fn(char) -> bool) -> bool {
    !word.is_empty() && word.chars().all(allowed)
}

/// Whether `text` is a snapshot time, `YYYYMMDDTHHMMSSZ`.
fn is_timestamp(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.len() == 16
        && bytes.iter().enumerate().all(|(index, byte)| match index {
            8 => *byte == b'T',
            15 => *byte == b'Z',
            _ => byte.is_ascii_digit(),
        })
}

/// The committed pin file.
fn pins_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("cell")
        .join("pins.txt")
}

/// Reads the committed pin file and parses it; returns the text and the
/// definition.
///
/// # Errors
///
/// Returns [`Failure::Io`] if the file cannot be read, and
/// [`Failure::Definition`] if it does not parse.
pub(crate) fn load() -> Result<(String, Definition), Failure> {
    let path = pins_path();
    let text =
        std::fs::read_to_string(&path).map_err(Failure::io("read the cell pin file", &path))?;
    let definition = parse(&text).map_err(Failure::Definition)?;
    Ok((text, definition))
}
