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
    let prefix = format!("{}/", input.root);
    let measured = measure(input, &prefix, &table_of, &mut violations);
    let rows = rows(input, &table_of, &measured, &mut violations);
    if !violations.is_empty() {
        return Err(violations);
    }
    Ok(Report {
        target: input.target.to_owned(),
        rows,
        uncovered: uncovered_functions(input, &prefix, &measured.placements),
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

/// The export's library files, summed per (table, crate).
#[derive(Default)]
struct Measured<'a> {
    figures: BTreeMap<(usize, &'a str), Figures>,
    /// Each placed file's absolute path and table.
    placements: BTreeMap<&'a str, Table>,
    /// Crates with at least one library file in the export.
    seen: BTreeSet<&'a str>,
}

/// Places each export file in its crate's table (or table 2 if generated).
/// A test file or an excluded file is a violation; files outside the
/// workspace, outside every library and binary code are left out.
fn measure<'a>(
    input: &Input<'a>,
    prefix: &str,
    table_of: &BTreeMap<&'a str, Table>,
    violations: &mut Vec<String>,
) -> Measured<'a> {
    let generated: Vec<Pattern> = input
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
        .collect();
    let mut measured = Measured::default();
    for file in &input.export.files {
        let Some(relative) = file.path.strip_prefix(prefix) else {
            continue;
        };
        if is_test_file(relative) {
            violations.push(format!("test file counted as covered code: {relative}"));
            continue;
        }
        if let Some(entry) = input
            .exclusions
            .iter()
            .find(|entry| entry.pattern.matches(relative))
        {
            violations.push(format!(
                "excluded file in the export: {relative} (excluded for: {})",
                entry.reason
            ));
            continue;
        }
        let Some((library, inside)) = library_of(input.libraries, relative) else {
            continue;
        };
        if inside == "main.rs" || inside.starts_with("bin/") {
            continue;
        }
        let Some(&crate_table) = table_of.get(library) else {
            continue;
        };
        measured.seen.insert(library);
        let table = if generated.iter().any(|pattern| pattern.matches(relative)) {
            Table::Two
        } else {
            crate_table
        };
        measured
            .figures
            .entry((index(table), library))
            .or_default()
            .add(&Figures {
                lines: file.lines,
                regions: file.regions,
                functions: file.functions,
            });
        measured.placements.insert(file.path.as_str(), table);
    }
    measured
}

/// One row per library in its table, plus a table 2 row for a crate with
/// generated files, sorted by table and crate. An expected crate with no
/// file in the export is a violation.
fn rows(
    input: &Input<'_>,
    table_of: &BTreeMap<&str, Table>,
    measured: &Measured<'_>,
    violations: &mut Vec<String>,
) -> Vec<(Table, Row)> {
    let mut rows = Vec::new();
    for library in input.libraries {
        let name = library.name.as_str();
        let Some(&table) = table_of.get(name) else {
            continue;
        };
        let not_built = input
            .tables
            .not_measured
            .iter()
            .any(|(crate_name, target)| *crate_name == name && *target == input.target);
        if !measured.seen.contains(name) && !not_built {
            violations.push(format!("expected crate `{name}` has no file in the export"));
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

/// Table 1's functions that never ran. A generic function's instantiations
/// share where they start, so they are grouped by file, line and column
/// (as LLVM groups them) and count as run if any instantiation ran.
fn uncovered_functions(
    input: &Input<'_>,
    prefix: &str,
    placements: &BTreeMap<&str, Table>,
) -> Vec<String> {
    let mut ran: BTreeMap<(&str, u64, u64), bool> = BTreeMap::new();
    for record in &input.export.functions {
        let placed = placements
            .get(record.file.as_str())
            .is_some_and(|table| *table == Table::One);
        if placed {
            *ran.entry((record.file.as_str(), record.line, record.column))
                .or_default() |= record.executed;
        }
    }
    ran.into_iter()
        .filter(|(_, executed)| !executed)
        .map(|((file, line, column), _)| {
            format!(
                "{}:{line}:{column}",
                file.strip_prefix(prefix).unwrap_or(file)
            )
        })
        .collect()
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
