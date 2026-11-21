// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The report's four tables (testing specification 5.4, point 3), the
//! checks that make an export usable, and the report text.
//!
//! The checks are about the export's form, never about a figure: a test
//! file counted as covered code, an excluded file present, an expected
//! crate missing, a crate in no table or in two, or an export written by
//! another tool version.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use super::TOOL_VERSION;
use super::exclusions::{Exclusion, Pattern, is_test_file};
use super::export::{Counts, Export};

#[cfg(test)]
mod tests;

/// Which table a crate's library code is reported in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Table {
    /// Hand-written code of the pure decoder and state-machine crates.
    One,
    /// Generated codec files; chosen by file pattern, never by crate.
    Two,
    /// `ironafs-client`, the drivers, `ironafs-os`, `ironafs-gss` and the
    /// operating-system layers.
    Three,
    /// Everything else.
    Four,
}

const ORDER: [(Table, &str); 4] = [
    (
        Table::One,
        "Table 1: pure decoder and state-machine crates, hand-written code",
    ),
    (
        Table::Two,
        "Table 2: generated codec files (report only, never ratcheted)",
    ),
    (
        Table::Three,
        "Table 3: `ironafs-client`, drivers, `ironafs-os`, `ironafs-gss` and operating-system layers (report only)",
    ),
    (
        Table::Four,
        "Table 4: `ironafs-time`, `ironafs-crypto`, `xtask`, `ironafs-sim`, `ironafs-test-support` and binaries' libraries (report only)",
    ),
];

/// The assignment the report uses.
#[derive(Debug, Clone, Copy)]
pub(super) struct Tables<'a> {
    /// Each crate and its table. A crate added to the architecture is
    /// added here in the same pull request.
    pub(super) crates: &'a [(&'a str, Table)],
    /// Patterns of generated codec files, reported in table 2 instead of
    /// their crate's table.
    pub(super) generated: &'a [&'a str],
    /// Crates whose library is not built on a target: (crate, target).
    pub(super) not_measured: &'a [(&'a str, &'a str)],
}

/// The repository's tables (testing specification 5.4, point 3).
pub(super) const TABLES: Tables<'static> = Tables {
    crates: &[
        ("ironafs-xdr", Table::One),
        ("ironafs-rx", Table::One),
        ("ironafs-rxkad", Table::One),
        ("ironafs-rxgk", Table::One),
        ("ironafs-proto", Table::One),
        ("ironafs-vfs", Table::One),
        ("ironafs-cache", Table::One),
        ("ironafs-auth", Table::One),
        ("ironafs-ipc", Table::One),
        ("ironafs-config", Table::One),
        ("ironafs-events", Table::One),
        ("ironafs-client", Table::Three),
        ("ironafs-net", Table::Three),
        ("ironafs-gss", Table::Three),
        ("ironafs-os", Table::Three),
        ("ironafs-fuse", Table::Three),
        ("ironafs-smb", Table::Three),
        ("ironafs-cloudfiles", Table::Three),
        ("ironafs-time", Table::Four),
        ("ironafs-crypto", Table::Four),
        ("ironafsd", Table::Four),
        ("ironafs-cli", Table::Four),
        ("xtask", Table::Four),
        ("ironafs-sim", Table::Four),
        ("ironafs-test-support", Table::Four),
    ],
    // The codec generator's pull request adds its output's pattern.
    generated: &[],
    not_measured: &[],
};

/// A workspace crate with a library target, and the directory that holds
/// its library code, relative to the workspace root.
#[derive(Debug)]
pub(super) struct Library {
    pub(super) name: String,
    pub(super) dir: String,
}

/// What the check reads.
#[derive(Debug)]
pub(super) struct Input<'a> {
    /// The absolute workspace root, as the export's file names start.
    pub(super) root: &'a str,
    pub(super) target: &'a str,
    pub(super) libraries: &'a [Library],
    pub(super) exclusions: &'a [Exclusion],
    pub(super) export: &'a Export,
    pub(super) tables: &'a Tables<'a>,
}

#[derive(Debug, Default, Clone, Copy)]
struct Figures {
    lines: Counts,
    regions: Counts,
    functions: Counts,
}

impl Figures {
    fn add(&mut self, other: &Figures) {
        self.lines.add(other.lines);
        self.regions.add(other.regions);
        self.functions.add(other.functions);
    }
}

#[derive(Debug)]
struct Row {
    name: String,
    /// `None`: not built on the target, so not measured.
    figures: Option<Figures>,
}

/// The report of one export.
#[derive(Debug)]
pub(super) struct Report {
    target: String,
    rows: Vec<(Table, Row)>,
    /// Table 1's uncovered functions, `file:line:column`, each once.
    uncovered: Vec<String>,
}

/// Checks an export and builds its report.
///
/// # Errors
///
/// Returns every violation found.
pub(super) fn check(input: &Input<'_>) -> Result<Report, Vec<String>> {
    let mut violations = Vec::new();
    if input.export.tool_version != TOOL_VERSION {
        violations.push(format!(
            "the export was written by cargo-llvm-cov {}, not the pinned {TOOL_VERSION}",
            input.export.tool_version
        ));
    }
    let table_of = assign_tables(input, &mut violations);
    let generated = generated_patterns(input, &mut violations);
    let places = Places {
        input,
        prefix: format!("{}/", input.root),
        table_of,
        generated,
    };
    let measured = measure(&places, &mut violations);
    let groups = function_groups(input);
    reconcile(&places, &measured, &groups, &mut violations);
    let rows = rows(&places, &measured, &mut violations);
    if measured.placements.is_empty() {
        violations.push("no library file of the workspace was measured".to_owned());
    }
    if !violations.is_empty() {
        return Err(violations);
    }
    Ok(Report {
        target: input.target.to_owned(),
        rows,
        uncovered: uncovered_functions(&places, &measured, &groups),
    })
}

/// Each workspace library's table; a library in no table or in two is a
/// violation.
fn assign_tables<'a>(input: &Input<'a>, violations: &mut Vec<String>) -> BTreeMap<&'a str, Table> {
    let mut table_of = BTreeMap::new();
    for library in input.libraries {
        let tables: Vec<Table> = input
            .tables
            .crates
            .iter()
            .filter(|(name, _)| *name == library.name)
            .map(|(_, table)| *table)
            .collect();
        match tables.as_slice() {
            [table] => {
                table_of.insert(library.name.as_str(), *table);
            }
            [] => violations.push(format!("crate `{}` is in no coverage table", library.name)),
            more => violations.push(format!(
                "crate `{}` is in {} coverage tables",
                library.name,
                more.len()
            )),
        }
    }
    table_of
}

fn generated_patterns(input: &Input<'_>, violations: &mut Vec<String>) -> Vec<Pattern> {
    input
        .tables
        .generated
        .iter()
        .filter_map(|text| match Pattern::parse(text) {
            Ok(pattern) => Some(pattern),
            Err(error) => {
                violations.push(format!("generated-file {error}"));
                None
            }
        })
        .collect()
}

/// What decides where a file's code is reported.
struct Places<'a> {
    input: &'a Input<'a>,
    /// The workspace root with a trailing `/`.
    prefix: String,
    table_of: BTreeMap<&'a str, Table>,
    generated: Vec<Pattern>,
}

/// Where one file's code is reported.
enum Place<'a> {
    /// The file must not be in the export at all.
    Refused(String),
    /// Outside the workspace, outside every library, or binary code.
    Elsewhere,
    Library {
        library: &'a str,
        table: Table,
    },
}

impl<'a> Places<'a> {
    fn relative<'p>(&self, path: &'p str) -> Option<&'p str> {
        path.strip_prefix(self.prefix.as_str())
    }

    fn place(&self, relative: &str) -> Place<'a> {
        if is_test_file(relative) {
            return Place::Refused(format!("test file counted as covered code: {relative}"));
        }
        if let Some(entry) = self
            .input
            .exclusions
            .iter()
            .find(|entry| entry.pattern.matches(relative))
        {
            return Place::Refused(format!(
                "excluded file in the export: {relative} (excluded for: {})",
                entry.reason
            ));
        }
        let Some((library, inside)) = library_of(self.input.libraries, relative) else {
            return Place::Elsewhere;
        };
        if inside == "main.rs" || inside.starts_with("bin/") {
            return Place::Elsewhere;
        }
        let Some(&crate_table) = self.table_of.get(library) else {
            return Place::Elsewhere;
        };
        let table = if self
            .generated
            .iter()
            .any(|pattern| pattern.matches(relative))
        {
            Table::Two
        } else {
            crate_table
        };
        Place::Library { library, table }
    }
}

/// The export's library files, summed per (table, crate).
#[derive(Default)]
struct Measured<'a> {
    figures: BTreeMap<(usize, &'a str), Figures>,
    /// Each placed file's absolute path, its table and its summary's
    /// function counts.
    placements: BTreeMap<&'a str, (Table, Counts)>,
    /// Crates with at least one hand-written (not generated) library file
    /// in the export.
    seen: BTreeSet<&'a str>,
    /// Instrumented lines per crate, generated files included.
    lines: BTreeMap<&'a str, u64>,
}

/// Places each export file in its crate's table (or table 2 if generated).
/// A test file or an excluded file is a violation.
fn measure<'a>(places: &Places<'a>, violations: &mut Vec<String>) -> Measured<'a> {
    let mut measured = Measured::default();
    for file in &places.input.export.files {
        let Some(relative) = places.relative(&file.path) else {
            continue;
        };
        let (library, table) = match places.place(relative) {
            Place::Refused(violation) => {
                violations.push(violation);
                continue;
            }
            Place::Elsewhere => continue,
            Place::Library { library, table } => (library, table),
        };
        if table != Table::Two {
            measured.seen.insert(library);
        }
        let lines = measured.lines.entry(library).or_default();
        *lines = lines.saturating_add(file.lines.count);
        measured
            .figures
            .entry((index(table), library))
            .or_default()
            .add(&Figures {
                lines: file.lines,
                regions: file.regions,
                functions: file.functions,
            });
        measured
            .placements
            .insert(file.path.as_str(), (table, file.functions));
    }
    measured
}

/// Function records grouped per file by start position, each group run if
/// any of its records ran: a generic function's instantiations (and its
/// copies in other test binaries) share a start, and LLVM groups them the
/// same way for each file's function count.
fn function_groups<'a>(input: &Input<'a>) -> BTreeMap<&'a str, BTreeMap<(u64, u64), bool>> {
    let mut groups: BTreeMap<&str, BTreeMap<(u64, u64), bool>> = BTreeMap::new();
    for record in &input.export.functions {
        *groups
            .entry(record.file.as_str())
            .or_default()
            .entry((record.line, record.column))
            .or_default() |= record.executed;
    }
    groups
}

/// Each placed file's function records must give exactly its summary's
/// function counts, and a library file with function records must have a
/// summary: otherwise records were dropped or the export is inconsistent.
fn reconcile(
    places: &Places<'_>,
    measured: &Measured<'_>,
    groups: &BTreeMap<&str, BTreeMap<(u64, u64), bool>>,
    violations: &mut Vec<String>,
) {
    for (path, (_, summary)) in &measured.placements {
        let found = groups.get(path).map_or(Counts::default(), |starts| Counts {
            count: u64::try_from(starts.len()).unwrap_or(u64::MAX),
            covered: u64::try_from(starts.values().filter(|ran| **ran).count()).unwrap_or(u64::MAX),
        });
        if found != *summary {
            violations.push(format!(
                "{}: function records give {}/{} run, its summary says {}/{}",
                places.relative(path).unwrap_or(path),
                found.covered,
                found.count,
                summary.covered,
                summary.count
            ));
        }
    }
    for path in groups.keys() {
        if measured.placements.contains_key(path) {
            continue;
        }
        if let Some(relative) = places.relative(path)
            && let Place::Library { .. } = places.place(relative)
        {
            violations.push(format!(
                "function record for {relative}, which has no file summary in the export"
            ));
        }
    }
}

/// One row per library in its table, plus a table 2 row for a crate with
/// generated files, sorted by table and crate. An expected crate with no
/// hand-written file, a measured crate with no instrumented line, and a
/// crate listed as not built that has files are violations.
fn rows(
    places: &Places<'_>,
    measured: &Measured<'_>,
    violations: &mut Vec<String>,
) -> Vec<(Table, Row)> {
    let input = places.input;
    let mut rows = Vec::new();
    for library in input.libraries {
        let name = library.name.as_str();
        let Some(&table) = places.table_of.get(name) else {
            continue;
        };
        let not_built = input
            .tables
            .not_measured
            .iter()
            .any(|(crate_name, target)| *crate_name == name && *target == input.target);
        let has_files = measured.lines.contains_key(name);
        if not_built && has_files {
            violations.push(format!(
                "crate `{name}` is listed as not built on {} but has files in the export",
                input.target
            ));
        } else if !not_built && !measured.seen.contains(name) {
            violations.push(format!("expected crate `{name}` has no file in the export"));
        } else if !not_built && measured.lines.get(name).is_some_and(|lines| *lines == 0) {
            violations.push(format!(
                "crate `{name}` has no instrumented line in the export"
            ));
        }
        let figures = (!not_built).then(|| {
            measured
                .figures
                .get(&(index(table), name))
                .copied()
                .unwrap_or_default()
        });
        rows.push((
            table,
            Row {
                name: name.to_owned(),
                figures,
            },
        ));
        if let Some(generated) = measured.figures.get(&(index(Table::Two), name)) {
            rows.push((
                Table::Two,
                Row {
                    name: name.to_owned(),
                    figures: Some(*generated),
                },
            ));
        }
    }
    rows.sort_by(|(a_table, a_row), (b_table, b_row)| {
        (index(*a_table), &a_row.name).cmp(&(index(*b_table), &b_row.name))
    });
    rows
}

fn index(table: Table) -> usize {
    ORDER
        .iter()
        .position(|(entry, _)| *entry == table)
        .unwrap_or(ORDER.len())
}

/// The library whose directory holds `relative` (the deepest one), and the
/// path inside that directory.
fn library_of<'a, 'b>(libraries: &'a [Library], relative: &'b str) -> Option<(&'a str, &'b str)> {
    libraries
        .iter()
        .filter_map(|library| {
            relative
                .strip_prefix(&library.dir)
                .and_then(|rest| rest.strip_prefix('/'))
                .map(|inside| (library, inside))
        })
        .max_by_key(|(library, _)| library.dir.len())
        .map(|(library, inside)| (library.name.as_str(), inside))
}

/// Table 1's functions that never ran, one per start position (see
/// [`function_groups`]). Functions a `macro_rules!` macro generates start
/// inside the macro's definition, so LLVM gives every expansion of one
/// definition the same start and counts them as one function; they are one
/// entry here too, run if any expansion ran.
fn uncovered_functions(
    places: &Places<'_>,
    measured: &Measured<'_>,
    groups: &BTreeMap<&str, BTreeMap<(u64, u64), bool>>,
) -> Vec<String> {
    let mut uncovered = Vec::new();
    for (path, starts) in groups {
        let in_table_one = measured
            .placements
            .get(path)
            .is_some_and(|(table, _)| *table == Table::One);
        if !in_table_one {
            continue;
        }
        let relative = places.relative(path).unwrap_or(path);
        for ((line, column), ran) in starts {
            if !ran {
                uncovered.push(format!("{relative}:{line}:{column}"));
            }
        }
    }
    uncovered
}

fn cell(counts: Counts) -> String {
    format!(
        "{}/{} ({} uncovered)",
        counts.covered,
        counts.count,
        counts.uncovered()
    )
}

impl Report {
    /// The report as Markdown.
    pub(super) fn render(&self) -> String {
        let mut text = String::new();
        let _ = writeln!(text, "# Coverage report\n");
        let _ = writeln!(
            text,
            "Tool: cargo-llvm-cov {TOOL_VERSION}. Target: {}. Library code only, per crate; \
             each figure is covered/total for that target.\n",
            self.target
        );
        let _ = writeln!(
            text,
            "- Branch coverage: not available. The tool's branch option needs an unstable \
             compiler option, and the pinned compiler is stable.\n\
             - Doc-tests: not measured. The tool needs a nightly compiler for them.\n\
             - Code behind a `cfg` this target does not build is not measured, not uncovered; \
             a crate not built on this target reads \"not measured\".\n\
             - No figure fails anything: the report only checks that the export is usable."
        );
        for (table, heading) in ORDER {
            let _ = writeln!(text, "\n## {heading}\n");
            let rows: Vec<&Row> = self
                .rows
                .iter()
                .filter(|(row_table, _)| *row_table == table)
                .map(|(_, row)| row)
                .collect();
            if rows.is_empty() {
                let _ = writeln!(text, "No crate of this table is in the workspace yet.");
                continue;
            }
            let _ = writeln!(
                text,
                "| Crate | Target | Lines | Regions | Functions | Branches |\n|---|---|---|---|---|---|"
            );
            for row in rows {
                let (lines, regions, functions) = match &row.figures {
                    Some(figures) => (
                        cell(figures.lines),
                        cell(figures.regions),
                        cell(figures.functions),
                    ),
                    None => (
                        "not measured".to_owned(),
                        "not measured".to_owned(),
                        "not measured".to_owned(),
                    ),
                };
                let _ = writeln!(
                    text,
                    "| {} | {} | {lines} | {regions} | {functions} | not available |",
                    row.name, self.target
                );
            }
            if table == Table::One && !self.uncovered.is_empty() {
                let _ = writeln!(
                    text,
                    "\nUncovered functions ({}, each once across instantiations):\n",
                    self.target
                );
                for function in &self.uncovered {
                    let _ = writeln!(text, "- {function}");
                }
            }
        }
        text
    }
}
