// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The part of a rustc argument vector that the release harness can reproduce.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};

use super::HARNESS;

#[cfg(test)]
mod tests;

pub(super) const DEFAULTS_RELEASE: &str = "1.100.0";
pub(super) const DEFAULTS_COMMIT: &str = "0ef1bc386059596dada8fb65b50425aab2ea637a";
pub(super) const TARGETS: [&str; 4] = [
    "x86_64-unknown-linux-gnu",
    "aarch64-unknown-linux-gnu",
    "x86_64-unknown-linux-musl",
    "aarch64-unknown-linux-musl",
];

const UNSUPPORTED_LTO: &str = "cross-crate or unrecognised LTO requires a linkable wipe harness";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Lto {
    #[default]
    Implicit,
    Off,
    CrossCrate,
}

#[derive(Debug, PartialEq, Eq)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "rule 13: each bool reflects an independent rustc switch"
)]
struct Codegen {
    edition: u16,
    opt_level: char,
    incremental: bool,
    units: u32,
    lto: Lto,
    local_thin_lto: bool,
    debug_assertions: bool,
    overflow_checks: bool,
    embed_bitcode: bool,
    panic: String,
    debuginfo: &'static str,
    strip: String,
    target_cpu: Option<String>,
    target_features: Vec<String>,
    cfgs: BTreeSet<String>,
}

#[derive(Debug)]
pub(super) struct Invocation {
    pub(super) name: String,
    pub(super) kind: String,
    pub(super) out_dir: PathBuf,
    pub(super) extra_filename: String,
    pub(super) externs: Vec<PathBuf>,
    codegen: Codegen,
}

impl Invocation {
    pub(super) fn units(&self) -> u32 {
        self.codegen.units
    }

    pub(super) fn local_thin_lto(&self) -> bool {
        self.codegen.local_thin_lto
    }

    pub(super) fn output_stem(&self) -> String {
        format!("{}{}", self.name, self.extra_filename)
    }

    pub(super) fn compare(&self, shipping: &Self) -> Result<(), String> {
        if self.codegen == shipping.codegen {
            return Ok(());
        }
        // State the difference without printing paths, flags or cfg values.
        let a = &self.codegen;
        let b = &shipping.codegen;
        let fields = [
            ("edition", a.edition == b.edition),
            ("opt-level", a.opt_level == b.opt_level),
            ("incremental", a.incremental == b.incremental),
            ("codegen-units", a.units == b.units),
            (
                "local ThinLTO",
                a.local_thin_lto == b.local_thin_lto && a.lto == b.lto,
            ),
            ("debug-assertions", a.debug_assertions == b.debug_assertions),
            ("overflow-checks", a.overflow_checks == b.overflow_checks),
            ("embed-bitcode", a.embed_bitcode == b.embed_bitcode),
            ("panic", a.panic == b.panic),
            ("debuginfo", a.debuginfo == b.debuginfo),
            ("strip", a.strip == b.strip),
            ("target-cpu", a.target_cpu == b.target_cpu),
            ("target-feature", a.target_features == b.target_features),
            ("explicit cfg", a.cfgs == b.cfgs),
        ];
        let differences: Vec<_> = fields
            .into_iter()
            .filter_map(|(field, same)| (!same).then_some(field))
            .collect();
        Err(format!(
            "{} is not built under the shipping profile (different {})",
            self.name,
            differences.join(", ")
        ))
    }
}

#[derive(Default)]
struct Arguments {
    source: Option<PathBuf>,
    name: Option<String>,
    kind: Option<String>,
    edition: Option<u16>,
    out_dir: Option<PathBuf>,
    target: Option<String>,
    emit: BTreeSet<String>,
    cfgs: BTreeSet<String>,
    externs: Vec<PathBuf>,
    settings: Settings,
}

#[derive(Default)]
struct Settings {
    values: BTreeMap<&'static str, String>,
    features: Vec<String>,
    lto: Lto,
    plugin_lto: bool,
    incremental: bool,
    extra_filename: String,
}

fn boolean(value: &str) -> Option<bool> {
    match value {
        "" | "y" | "yes" | "on" | "true" => Some(true),
        "n" | "no" | "off" | "false" => Some(false),
        _ => None,
    }
}

fn debug_level(value: &str) -> Option<&'static str> {
    match value {
        "0" | "none" => Some("none"),
        "line-directives-only" => Some("line-directives-only"),
        "line-tables-only" => Some("line-tables-only"),
        "1" | "limited" => Some("limited"),
        "2" | "full" => Some("full"),
        _ => None,
    }
}

fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

impl Settings {
    fn add(&mut self, option: &str, dirs: &[&Path]) -> Result<(), &'static str> {
        let (key, argument) = option
            .split_once('=')
            .map_or((option, None), |(key, value)| (key, Some(value)));
        let value = argument.unwrap_or_default();
        // rustc accepts underscores as well as hyphens in codegen keys.
        let key = key.replace('_', "-");
        if argument == Some("") {
            return Err(match key.as_str() {
                "lto" | "linker-plugin-lto" => UNSUPPORTED_LTO,
                _ => "empty rustc codegen option value",
            });
        }
        let singleton = match key.as_str() {
            "metadata" if !value.is_empty() => return Ok(()),
            "extra-filename" if value.strip_prefix('-').is_some_and(identifier) => {
                value.clone_into(&mut self.extra_filename);
                return Ok(());
            }
            // Even a path named "off" enables the incremental cache.
            "incremental" if !value.is_empty() => {
                if !dirs.iter().any(|dir| inside(Path::new(value), dir)) {
                    return Err("rustc incremental cache is outside the fresh build");
                }
                self.incremental = true;
                return Ok(());
            }
            "lto" => {
                self.lto = match value {
                    "thin" | "fat" => Lto::CrossCrate,
                    _ => match boolean(value) {
                        Some(true) => Lto::CrossCrate,
                        Some(false) => Lto::Off,
                        None => return Err(UNSUPPORTED_LTO),
                    },
                };
                return Ok(());
            }
            "linker-plugin-lto" => {
                // rustc also accepts a plugin path here.
                self.plugin_lto = boolean(value) != Some(false);
                return Ok(());
            }
            "target-feature" if !value.is_empty() => {
                for feature in value.split(',') {
                    if !feature.starts_with(['+', '-'])
                        || !feature.get(1..).is_some_and(target_option)
                    {
                        return Err("unsupported rustc target-feature value");
                    }
                    self.features.push(feature.to_owned());
                }
                return Ok(());
            }
            "opt-level" if matches!(value, "0" | "1" | "2" | "3" | "s" | "z") => "opt-level",
            "codegen-units" if value.parse::<u32>().is_ok_and(|units| units > 0) => "codegen-units",
            "debug-assertions" if boolean(value).is_some() => "debug-assertions",
            "overflow-checks" if boolean(value).is_some() => "overflow-checks",
            "embed-bitcode" if boolean(value).is_some() => "embed-bitcode",
            "panic" if matches!(value, "abort" | "unwind") => "panic",
            "debuginfo" if debug_level(value).is_some() => "debuginfo",
            "strip" if matches!(value, "none" | "debuginfo" | "symbols") => "strip",
            "target-cpu" if value != "native" && target_option(value) => "target-cpu",
            _ => return Err("unsupported rustc codegen option"),
        };
        self.values.insert(singleton, value.to_owned());
        Ok(())
    }

    fn effective(
        self,
        edition: u16,
        verified_defaults: bool,
        emitting: bool,
    ) -> Result<Codegen, &'static str> {
        if self.lto == Lto::CrossCrate || self.plugin_lto {
            return Err(UNSUPPORTED_LTO);
        }
        let value = |key| self.values.get(key).map(String::as_str);
        let opt_level = value("opt-level")
            .and_then(|value| value.chars().next())
            .unwrap_or('0');
        let units = match value("codegen-units") {
            Some(value) => value
                .parse()
                .map_err(|_| "invalid rustc codegen-unit count")?,
            None if emitting => return Err("assembly invocation has no explicit codegen units"),
            None if !verified_defaults => {
                return Err("implicit codegen units are unverified for this compiler and target");
            }
            None if self.incremental => 256,
            None => 16,
        };
        if self.lto == Lto::Implicit && !verified_defaults {
            return Err("implicit local ThinLTO is unverified for this compiler and target");
        }
        let debug_assertions = value("debug-assertions")
            .and_then(boolean)
            .unwrap_or(opt_level == '0');
        Ok(Codegen {
            edition,
            opt_level,
            incremental: self.incremental,
            units,
            lto: self.lto,
            local_thin_lto: self.lto == Lto::Implicit && opt_level != '0' && units > 1,
            debug_assertions,
            overflow_checks: value("overflow-checks")
                .and_then(boolean)
                .unwrap_or(debug_assertions),
            embed_bitcode: value("embed-bitcode").and_then(boolean).unwrap_or(true),
            panic: value("panic").unwrap_or("unwind").to_owned(),
            debuginfo: value("debuginfo").and_then(debug_level).unwrap_or("none"),
            strip: value("strip").unwrap_or("none").to_owned(),
            target_cpu: value("target-cpu").map(str::to_owned),
            target_features: self.features,
            cfgs: BTreeSet::new(),
        })
    }
}

fn target_option(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
}

/// A Cargo-controlled path within one of the supplied fresh directories.
pub(super) fn inside(path: &Path, dir: &Path) -> bool {
    path.strip_prefix(dir).is_ok_and(|relative| {
        relative.components().next().is_some()
            && relative
                .components()
                .all(|part| matches!(part, Component::Normal(_)))
    })
}

fn once<T>(slot: &mut Option<T>, value: T) -> Result<(), &'static str> {
    if slot.replace(value).is_some() {
        return Err("duplicate rustc invocation identity argument");
    }
    Ok(())
}

fn split_option(word: &str) -> (&str, Option<&str>) {
    if word.starts_with("--") {
        return word
            .split_once('=')
            .map_or((word, None), |(key, value)| (key, Some(value)));
    }
    for key in ["-C", "-L", "-A", "-W", "-D", "-F"] {
        if let Some(value) = word.strip_prefix(key) {
            return (key, (!value.is_empty()).then_some(value));
        }
    }
    (word, None)
}

impl Arguments {
    fn option(&mut self, key: &str, value: &str, dirs: &[&Path]) -> Result<(), &'static str> {
        match key {
            "--target" => once(&mut self.target, value.to_owned()),
            "--crate-name" if identifier(value) => once(&mut self.name, value.to_owned()),
            "--crate-type" if matches!(value, "lib" | "rlib" | "bin") => {
                once(&mut self.kind, value.to_owned())
            }
            "--edition" if matches!(value, "2015" | "2018" | "2021" | "2024") => once(
                &mut self.edition,
                value.parse().map_err(|_| "invalid rustc edition")?,
            ),
            "--out-dir" if dirs.iter().any(|dir| inside(Path::new(value), dir)) => {
                once(&mut self.out_dir, PathBuf::from(value))
            }
            "--emit" => {
                for kind in value.split(',') {
                    if kind.contains('=') {
                        return Err("explicit rustc output file can change the codegen-unit count");
                    }
                    if !matches!(kind, "dep-info" | "metadata" | "link" | "asm") {
                        return Err("unsupported rustc emit option");
                    }
                    self.emit.insert(kind.to_owned());
                }
                Ok(())
            }
            "--cfg" if !value.is_empty() => {
                self.cfgs.insert(value.to_owned());
                Ok(())
            }
            "--extern" => {
                let (name, path) = value
                    .split_once('=')
                    .ok_or("rustc extern has no artifact path")?;
                let path = Path::new(path);
                if !identifier(name) || !dirs.iter().any(|dir| inside(path, dir)) {
                    return Err("rustc extern is outside the fresh build");
                }
                self.externs.push(path.to_path_buf());
                Ok(())
            }
            "-L" if value
                .strip_prefix("dependency=")
                .is_some_and(|path| dirs.iter().any(|dir| inside(Path::new(path), dir))) =>
            {
                Ok(())
            }
            "--check-cfg" if value.starts_with("cfg(") && value.ends_with(')') => Ok(()),
            "--error-format" if value == "json" => Ok(()),
            "--json"
                if value.split(',').all(|kind| {
                    matches!(
                        kind,
                        "diagnostic-rendered-ansi"
                            | "artifacts"
                            | "future-incompat"
                            | "unused-externs-silent"
                    )
                }) =>
            {
                Ok(())
            }
            "--cap-lints" if matches!(value, "allow" | "warn" | "deny" | "forbid") => Ok(()),
            "-A" | "-W" | "-D" | "-F" | "--allow" | "--warn" | "--deny" | "--forbid"
            | "--force-warn"
                if !value.is_empty() && value.split("::").all(target_option) =>
            {
                Ok(())
            }
            "-C" => self.settings.add(value, dirs),
            _ => Err("unsupported rustc argument"),
        }
    }
}

/// Classifies the actual invocation. Unclassified flags cannot become proof
/// by disappearing from the comparison.
pub(super) fn classify(
    argv: &[String],
    expected_target: &str,
    dirs: &[&Path],
    verified_compiler: bool,
) -> Result<Invocation, &'static str> {
    let mut found = Arguments::default();
    let mut words = argv.iter().map(String::as_str);
    if words.clone().any(|word| word.starts_with('@')) {
        return Err("rustc response files are unsupported");
    }
    while let Some(word) = words.next() {
        if word == "-o" || word.starts_with("-o") && !word.starts_with("--") {
            return Err("explicit rustc output file can change the codegen-unit count");
        }
        if !word.starts_with('-') && Path::new(word).extension().is_some_and(|ext| ext == "rs") {
            once(&mut found.source, PathBuf::from(word))?;
        } else {
            let (key, attached) = split_option(word);
            let value = attached
                .or_else(|| words.next())
                .ok_or("rustc option has no value")?;
            found.option(key, value, dirs)?;
        }
    }
    if found.target.as_deref() != Some(expected_target) || !TARGETS.contains(&expected_target) {
        return Err("rustc invocation has the wrong target");
    }
    found.source.ok_or("rustc invocation has no source")?;
    let name = found.name.ok_or("rustc invocation has no crate name")?;
    let kind = found.kind.ok_or("rustc invocation has no crate type")?;
    let emitting = name == HARNESS;
    if !found.emit.contains("link") || found.emit.contains("asm") != emitting {
        return Err("rustc invocation has the wrong build stage");
    }
    let extra_filename = std::mem::take(&mut found.settings.extra_filename);
    let mut codegen =
        found
            .settings
            .effective(found.edition.unwrap_or(2015), verified_compiler, emitting)?;
    codegen.cfgs = found.cfgs;
    Ok(Invocation {
        name,
        kind,
        out_dir: found
            .out_dir
            .ok_or("rustc invocation has no fresh output directory")?,
        extra_filename,
        externs: found.externs,
        codegen,
    })
}
