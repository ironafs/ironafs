// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! `crates/ironafs-os/sites.json`, the machine-readable copy of ADR 0005's
//! closed list (point 6.7), and its own rules.
//!
//! The copy holds every row of the ADR's table. Its text columns are
//! compared with the table in the ADR itself, cell for cell, so the copy
//! cannot drift from the decision unseen. Each row has a status per
//! target, *approved* or *implemented*; an implemented row also records the
//! code it holds: its module, the C headers its declarations come from,
//! its extern blocks with their functions and `#[link]`, its `unsafe`
//! blocks, each with its kind and one operation, and its count of `unsafe
//! impl`s. `check-os` holds the code to exactly that.

use std::collections::BTreeMap;

use super::ALLOWLIST;
use super::scan::ExternItem;
use crate::json::{self, Value};

#[cfg(test)]
mod tests;

/// The ADR whose table the copy holds.
pub(crate) const ADR: &str = "docs/adr/0005-ironafs-os.md";

/// The heading of the section that holds the table.
const TABLE_SECTION: &str = "### 2. The closed list";

/// A row's state on one target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Status {
    /// In the ADR's table; no code may exist for it on this target.
    Approved,
    /// Its code exists on this target and must match the row exactly.
    Implemented,
}

/// The four things an `unsafe` block may hold (ADR 0005, point 6.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BlockKind {
    /// One foreign call, of a function the row declares.
    Call,
    /// One bounded read of memory a foreign call wrote.
    Read,
    /// One adoption of ownership from a raw value.
    Adopt,
    /// One volatile write of a zero byte.
    Write,
}

/// Operations each kind other than a call may name.
const READS: &[&str] = &["read_unaligned"];
const ADOPTIONS: &[&str] = &["from_raw_fd", "from_raw_handle", "from_raw"];
/// The one write: `core`'s own volatile write, by its whole path, never a
/// same-named item elsewhere. That `core` here is the real crate is
/// `check-os`'s rule that no file of the crate names an item `core`.
const WRITES: &[&str] = &["core::ptr::write_volatile", "::core::ptr::write_volatile"];

/// One `unsafe` block of an implemented row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Block {
    pub(crate) kind: BlockKind,
    /// The path the block calls, as written: `abi::syscall`.
    pub(crate) operation: String,
    /// Its arguments, as `check-os` reads them: each argument's tokens
    /// joined by spaces. Recording them binds the call's shape, so a
    /// different argument count, or a different operand, is a mismatch.
    pub(crate) arguments: Vec<String>,
}

impl Block {
    /// The function or method the block calls: its path's last segment.
    pub(crate) fn callee(&self) -> &str {
        self.operation.rsplit("::").next().unwrap_or_default()
    }
}

/// One extern block of an implemented row: its `#[link]` arguments as
/// `check-os` reads them, and its functions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ExternBlock {
    pub(crate) link: Option<String>,
    pub(crate) items: Vec<ExternItem>,
}

/// The code of an implemented row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Code {
    /// The module under `src/` that holds every site of the row: the file
    /// `<module>.rs` and the files under `<module>/`.
    pub(crate) module: String,
    /// The C headers the row's declarations are written from, for the ABI
    /// check (ADR 0005, point 5.6).
    pub(crate) headers: Vec<String>,
    pub(crate) extern_blocks: Vec<ExternBlock>,
    pub(crate) blocks: Vec<Block>,
    pub(crate) impls: usize,
    /// The C facts the row's declaration files must state, by name, beside
    /// its functions: its structs and its constants. `check-abi` requires
    /// each among the declarations the target compiles, so a fact moved out
    /// of the module cannot drop out of the proof unseen.
    pub(crate) structs: Vec<String>,
    pub(crate) constants: Vec<String>,
    /// The row's Miri stand-in (ADR 0005, point 6.5), if it has one: how
    /// many `unsafe fn`s it defines and its `unsafe` blocks, each a call as
    /// written with its arguments. `check-os` holds `stand_in.rs` to it.
    pub(crate) stand_in: Option<StandIn>,
}

/// What a row's Miri stand-in holds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct StandIn {
    pub(crate) functions: usize,
    /// Each block's call and arguments, as `(operation, arguments)`.
    pub(crate) blocks: Vec<(String, Vec<String>)>,
}

/// One row of the copy.
#[derive(Clone, Debug)]
pub(crate) struct Row {
    pub(crate) number: String,
    pub(crate) id: String,
    /// Targets, foreign item, linked from, declared, `unsafe` blocks: the
    /// ADR table's text, as written there.
    pub(crate) columns: [String; 5],
    /// The ADR leaves the row's items or block count open, so it cannot be
    /// implemented until a change fixes them (point 6.7).
    pub(crate) open: bool,
    pub(crate) status: BTreeMap<String, Status>,
    pub(crate) code: Option<Code>,
}

impl Row {
    /// Whether the row is implemented on `target`.
    pub(crate) fn implemented_on(&self, target: &str) -> bool {
        self.status.get(target) == Some(&Status::Implemented)
    }
}

/// The whole copy.
#[derive(Clone, Debug)]
pub(crate) struct Inventory {
    pub(crate) rows: Vec<Row>,
}

/// One row of the ADR's table.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct AdrRow {
    pub(crate) number: String,
    pub(crate) id: String,
    pub(crate) columns: [String; 5],
}

/// Reads the copy.
///
/// # Errors
///
/// Returns what is missing or malformed, naming the row.
pub(crate) fn parse(text: &str) -> Result<Inventory, String> {
    let document = json::parse(text).map_err(|error| format!("sites.json is not JSON: {error}"))?;
    let rows = document
        .get("rows")
        .and_then(Value::as_array)
        .ok_or("sites.json has no `rows` array")?;
    let rows = rows
        .iter()
        .enumerate()
        .map(|(index, row)| {
            parse_row(row).map_err(|error| format!("sites.json row {}: {error}", index + 1))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Inventory { rows })
}

fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str, String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("`{key}` is missing or not a string"))
}

fn flag(value: &Value, key: &str) -> Result<bool, String> {
    match value.get(key) {
        Some(Value::Bool(flag)) => Ok(*flag),
        _ => Err(format!("`{key}` is missing or not true or false")),
    }
}

fn array<'a>(value: &'a Value, key: &str) -> Result<&'a [Value], String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("`{key}` is missing or not an array"))
}

fn parse_row(row: &Value) -> Result<Row, String> {
    let number = match row.get("row") {
        Some(Value::Number(number)) => number.clone(),
        _ => return Err("`row` is missing or not a number".to_owned()),
    };
    let columns = [
        text(row, "targets")?.to_owned(),
        text(row, "foreign_item")?.to_owned(),
        text(row, "linked_from")?.to_owned(),
        text(row, "declared")?.to_owned(),
        text(row, "blocks")?.to_owned(),
    ];
    let mut status = BTreeMap::new();
    match row.get("status") {
        Some(Value::Object(members)) => {
            for (target, value) in members {
                let found = match value.as_str() {
                    Some("approved") => Status::Approved,
                    Some("implemented") => Status::Implemented,
                    _ => return Err(format!("status of {target} is not approved or implemented")),
                };
                if status.insert(target.clone(), found).is_some() {
                    return Err(format!("status of {target} is given twice"));
                }
            }
        }
        _ => return Err("`status` is missing or not an object".to_owned()),
    }
    let code = row.get("code").map(parse_code).transpose()?;
    Ok(Row {
        number,
        id: text(row, "id")?.to_owned(),
        columns,
        open: flag(row, "open")?,
        status,
        code,
    })
}

fn parse_code(code: &Value) -> Result<Code, String> {
    let strings = |key: &str| -> Result<Vec<String>, String> {
        array(code, key)?
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .map(str::to_owned)
                    .ok_or(format!("`{key}` holds a non-string"))
            })
            .collect()
    };
    let extern_blocks = array(code, "extern_blocks")?
        .iter()
        .map(|block| {
            let link = match block.get("link") {
                Some(Value::Null) => None,
                Some(Value::String(link)) => Some(link.clone()),
                _ => return Err("an extern block's `link` is missing".to_owned()),
            };
            let items = array(block, "items")?
                .iter()
                .map(|item| {
                    Ok(ExternItem {
                        name: text(item, "name")?.to_owned(),
                        safe: flag(item, "safe")?,
                    })
                })
                .collect::<Result<Vec<_>, String>>()?;
            Ok(ExternBlock { link, items })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let blocks = array(code, "blocks")?
        .iter()
        .map(|block| {
            let kind = match text(block, "kind")? {
                "call" => BlockKind::Call,
                "read" => BlockKind::Read,
                "adopt" => BlockKind::Adopt,
                "write" => BlockKind::Write,
                other => {
                    return Err(format!(
                        "block kind `{other}` is not call, read, adopt or write"
                    ));
                }
            };
            let arguments = array(block, "arguments")?
                .iter()
                .map(|value| {
                    value
                        .as_str()
                        .map(str::to_owned)
                        .ok_or("`arguments` holds a non-string".to_owned())
                })
                .collect::<Result<Vec<_>, String>>()?;
            Ok(Block {
                kind,
                operation: text(block, "operation")?.to_owned(),
                arguments,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let impls = match code.get("impls") {
        Some(Value::Number(number)) => number
            .parse()
            .map_err(|_| "`impls` is not a count".to_owned())?,
        _ => return Err("`impls` is missing or not a number".to_owned()),
    };
    let facts = code.get("c_facts").ok_or("`c_facts` is missing")?;
    let names = |key: &str| -> Result<Vec<String>, String> {
        array(facts, key)?
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .map(str::to_owned)
                    .ok_or(format!("`c_facts.{key}` holds a non-string"))
            })
            .collect()
    };
    let stand_in = parse_stand_in(code)?;
    Ok(Code {
        module: text(code, "module")?.to_owned(),
        headers: strings("headers")?,
        extern_blocks,
        blocks,
        impls,
        structs: names("structs")?,
        constants: names("constants")?,
        stand_in,
    })
}

/// A row's recorded Miri stand-in, if any.
fn parse_stand_in(code: &Value) -> Result<Option<StandIn>, String> {
    let stand_in = match code.get("stand_in") {
        None | Some(Value::Null) => return Ok(None),
        Some(stand_in) => stand_in,
    };
    let functions = match stand_in.get("functions") {
        Some(Value::Number(number)) => number
            .parse()
            .map_err(|_| "`stand_in.functions` is not a count".to_owned())?,
        _ => return Err("`stand_in.functions` is missing".to_owned()),
    };
    let blocks = array(stand_in, "blocks")?
        .iter()
        .map(|block| {
            let arguments = array(block, "arguments")?
                .iter()
                .map(|value| {
                    value
                        .as_str()
                        .map(str::to_owned)
                        .ok_or("a non-string argument".to_owned())
                })
                .collect::<Result<Vec<_>, String>>()?;
            Ok((text(block, "operation")?.to_owned(), arguments))
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(Some(StandIn { functions, blocks }))
}

/// The rows of the ADR's table: the lines starting `| <number> |` in the
/// section that holds it, split at each `|` not escaped as `\|`, with the
/// escapes kept as written.
pub(crate) fn adr_rows(text: &str) -> Vec<AdrRow> {
    let mut rows = Vec::new();
    let mut inside = false;
    for line in text.lines() {
        if line.starts_with("### ") {
            inside = line == TABLE_SECTION;
            continue;
        }
        if !inside {
            continue;
        }
        let cells = split_cells(line);
        let [number, id, rest @ ..] = cells.as_slice() else {
            continue;
        };
        let (true, Some(id)) = (
            !number.is_empty() && number.bytes().all(|byte| byte.is_ascii_digit()),
            id.strip_prefix('`').and_then(|id| id.strip_suffix('`')),
        ) else {
            continue;
        };
        if let [targets, foreign, linked, declared, blocks] = rest {
            rows.push(AdrRow {
                number: number.clone(),
                id: id.to_owned(),
                columns: [
                    targets.clone(),
                    foreign.clone(),
                    linked.clone(),
                    declared.clone(),
                    blocks.clone(),
                ],
            });
        }
    }
    rows
}

/// The trimmed cells of a Markdown table line, or nothing if it is not one.
fn split_cells(line: &str) -> Vec<String> {
    let Some(inner) = line
        .strip_prefix('|')
        .and_then(|rest| rest.strip_suffix('|'))
    else {
        return Vec::new();
    };
    let mut cells = Vec::new();
    let mut cell = String::new();
    let mut escaped = false;
    for character in inner.chars() {
        if character == '|' && !escaped {
            cells.push(cell.trim().to_owned());
            cell.clear();
        } else {
            cell.push(character);
        }
        escaped = character == '\\' && !escaped;
    }
    cells.push(cell.trim().to_owned());
    cells
}

/// The copy's departures from the ADR's table and from its own rules.
pub(crate) fn problems(inventory: &Inventory, adr: &str) -> Vec<String> {
    let mut found = Vec::new();
    let table = adr_rows(adr);
    if table.is_empty() {
        found.push(format!("{ADR} has no table under \"{TABLE_SECTION}\""));
    }
    if table.len() != inventory.rows.len() {
        found.push(format!(
            "sites.json has {} rows and the ADR's table {}: the copy holds every row",
            inventory.rows.len(),
            table.len()
        ));
    }
    for (row, adr_row) in inventory.rows.iter().zip(&table) {
        if row.number != adr_row.number || row.id != adr_row.id || row.columns != adr_row.columns {
            found.push(format!(
                "sites.json row {} ({}) differs from the ADR's row {} ({}): the copy keeps the \
                 table's text as written",
                row.number, row.id, adr_row.number, adr_row.id
            ));
        }
    }
    for row in &inventory.rows {
        found.extend(row_problems(row));
    }
    found
}

/// One row's departures from the copy's rules.
fn row_problems(row: &Row) -> Vec<String> {
    let mut found = Vec::new();
    let id = &row.id;
    for target in row.status.keys() {
        if !ALLOWLIST.contains(&target.as_str()) {
            found.push(format!(
                "{id}: {target} is not a target of ADR 0005, point 5.3"
            ));
        }
    }
    for (musl, gnu) in [
        ("x86_64-unknown-linux-musl", "x86_64-unknown-linux-gnu"),
        ("aarch64-unknown-linux-musl", "aarch64-unknown-linux-gnu"),
    ] {
        if row.status.get(musl) != row.status.get(gnu) {
            found.push(format!(
                "{id}: {musl} and {gnu} differ; a musl build compiles the glibc build's code"
            ));
        }
    }
    let implemented = row
        .status
        .values()
        .any(|status| *status == Status::Implemented);
    match (&row.code, implemented) {
        (None, true) => found.push(format!("{id}: implemented, but its code is not recorded")),
        (Some(_), false) => found.push(format!(
            "{id}: code is recorded, but it is implemented nowhere"
        )),
        _ => {}
    }
    let [_, foreign, ..] = &row.columns;
    if row.open != foreign.contains("open owner decision") {
        found.push(format!(
            "{id}: `open` is {}, but the ADR's row {} an open owner decision",
            row.open,
            if row.open { "names no" } else { "names" }
        ));
    }
    if row.open && implemented {
        found.push(format!(
            "{id}: the ADR leaves this row open, so it cannot be implemented until a change \
             fixes it (point 6.7)"
        ));
    }
    if let Some(code) = &row.code {
        let declared = |name: &str| {
            code.extern_blocks
                .iter()
                .flat_map(|block| &block.items)
                .any(|item| item.name == name)
        };
        for block in &code.blocks {
            let callee = block.callee();
            let fits = match block.kind {
                BlockKind::Call => declared(callee),
                BlockKind::Read => READS.contains(&callee),
                BlockKind::Adopt => ADOPTIONS.contains(&callee),
                BlockKind::Write => {
                    WRITES.contains(&block.operation.as_str())
                        && block.arguments.last().map(String::as_str) == Some("0")
                }
            };
            if !fits {
                found.push(format!(
                    "{id}: a {:?} block cannot hold `{}({})`",
                    block.kind,
                    block.operation,
                    block.arguments.join(", ")
                ));
            }
        }
        found.extend(adr_binding_problems(row, code));
    }
    found
}

/// How an implemented row's code departs from what the ADR's own row
/// approves: every declared function and every block's callee is named in
/// the row's foreign-item or block column; a block with no `#[link]` is
/// linked from the C library or libSystem, and a `#[link]` names the
/// library the row does; the `safe fn` declarations are those the declared
/// column allows; a row whose block column says "one" holds one site in
/// `unsafe`; and the copy's open flag is the ADR's (an open owner
/// decision). The prose of other block counts is bound by review.
fn adr_binding_problems(row: &Row, code: &Code) -> Vec<String> {
    let mut found = Vec::new();
    let id = &row.id;
    let [_, foreign, linked, declared, blocks] = &row.columns;
    let approved = |name: &str| {
        foreign.contains(&format!("{name}("))
            || foreign.contains(&format!("`{name}`"))
            || foreign.contains(&format!("::{name}`"))
            || blocks.contains(&format!("::{name}`"))
    };
    for item in code.extern_blocks.iter().flat_map(|block| &block.items) {
        if !approved(&item.name) {
            found.push(format!(
                "{id}: declares `{}`, which the ADR's row does not name (point 7.1)",
                item.name
            ));
        }
        let allowed = match declared.as_str() {
            "`safe fn`" => item.safe,
            "`unsafe fn`" => !item.safe,
            _ => true,
        };
        if !allowed {
            found.push(format!(
                "{id}: `{}` is declared {}, which the ADR's row does not allow ({declared})",
                item.name,
                if item.safe { "safe" } else { "unsafe" }
            ));
        }
    }
    for block in &code.blocks {
        if !approved(block.callee()) {
            found.push(format!(
                "{id}: a block calls `{}`, which the ADR's row does not name (point 7.1)",
                block.operation
            ));
        }
    }
    for block in &code.extern_blocks {
        let fits = match &block.link {
            None => linked.contains("C library") || linked.contains("libSystem"),
            Some(link) => link
                .split(';')
                .filter_map(|pair| pair.strip_prefix("name="))
                .all(|name| linked.contains(name) || linked == "see point 5.4"),
        };
        if !fits {
            found.push(format!(
                "{id}: an extern block linked as {:?}, where the ADR's row says \"{linked}\"",
                block.link
            ));
        }
    }
    // Blocks and impls are bound apart. Only the row whose block column
    // names `unsafe impl`s may hold any (ADR 0005, point 6.3: the two of
    // windows-dns); a row whose column says "one" holds one block.
    if code.impls > 0 && !blocks.contains("`unsafe impl`") {
        found.push(format!(
            "{id}: records {} unsafe impls; the ADR's row names none (point 6.3)",
            code.impls
        ));
    }
    let says_one = blocks == "one" || blocks.starts_with("one,") || blocks.starts_with("one (");
    if says_one && code.blocks.len() != 1 {
        found.push(format!(
            "{id}: holds {} unsafe blocks; the ADR's row says \"{blocks}\"",
            code.blocks.len()
        ));
    }
    // A row whose foreign item is the volatile write holds that write, of
    // zero, as a write block (sheet 24).
    let volatile = foreign.contains("write_volatile");
    let writes = code.blocks.iter().any(|block| {
        block.kind == BlockKind::Write
            && WRITES.contains(&block.operation.as_str())
            && block.arguments.last().map(String::as_str) == Some("0")
    });
    if volatile && !writes {
        found.push(format!(
            "{id}: the ADR's row is the volatile zero write, but no write block of zero is recorded"
        ));
    }
    // `syscall` reads up to six `long`s after the number, so a call passes
    // all six (the declaration's argument, in `linux_random::abi`); the
    // count is held here, not in the editable copy. What this establishes
    // is the operand count only. That each operand is a `c_long` is the
    // production code's own typing (the number a `c_long`, the six others
    // the elements of a `[c_long; 6]`), which the compiler checks and
    // review reads; this check does not prove operand types.
    for block in code
        .blocks
        .iter()
        .filter(|block| block.callee() == "syscall")
    {
        if block.arguments.len() != SYSCALL_ARGUMENTS {
            found.push(format!(
                "{id}: calls syscall with {} arguments; a syscall call passes the number and six \
                 longs ({SYSCALL_ARGUMENTS})",
                block.arguments.len()
            ));
        }
    }
    found
}

/// The arguments of every `syscall` call: the number and six `long`s.
const SYSCALL_ARGUMENTS: usize = 7;
