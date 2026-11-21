// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The node table against a reference model, over random operation
//! sequences and over every short sequence of a small domain.
//!
//! The model keeps what the table returned and what each context pins and
//! binds, and checks after every operation:
//! - every id returned for a new key is larger than every id returned
//!   before, so no id is ever reused (C11, point 2);
//! - the reverse index names exactly the live nodes returned for each file
//!   id (C11, point 3);
//! - every pinned node and every bound root is still in the table (C11,
//!   point 10), and every live node's parent, containing root and anchor
//!   are too (namespace specification, 5.6.1, point 8);
//! - each context's charge is the number of distinct nodes its pins and
//!   bindings reach (5.6.1, point 8), and a refused operation changes no
//!   charge and makes or evicts no record;
//! - the table never holds more records than its limit.

use std::collections::{BTreeMap, BTreeSet};

use ironafs_cache::{
    CellIndex, DYNAMIC_ROOT, FileId, Limits, NodeKind, NodeTable, PathKind, Uniquifier, Version,
    Vnode, VolumeId,
};
use ironafs_test_support::{Exhaustive, Property, Source};
use ironafs_vfs::NodeId;

use crate::fixture::context;

/// One operation; indices pick among the nodes live when it runs.
#[derive(Clone, Copy, Debug)]
enum Op {
    Entry {
        cell: u32,
        read_write: bool,
    },
    Child {
        pick: usize,
        vnode: u32,
    },
    Cross {
        context: u64,
        pick: usize,
        volume: u32,
        budget: u32,
    },
    Pin {
        context: u64,
        pick: usize,
        budget: u32,
    },
    Unpin {
        context: u64,
        pick: usize,
    },
    Unbind {
        context: u64,
        pick: usize,
    },
    Release {
        context: u64,
    },
}

#[derive(Debug)]
struct Case {
    nodes: u32,
    ops: Vec<Op>,
}

/// One operation over `picks + 1` picks, `contexts` contexts, `cells`
/// cells, `vnodes` vnodes and budgets up to `budget_max`.
fn op(
    source: &mut Source,
    picks: usize,
    contexts: u64,
    cells: u32,
    vnodes: u32,
    budget_max: u64,
) -> Op {
    match source.usize_in(0..=6) {
        0 => Op::Entry {
            cell: u32::try_from(source.u64_in(0..=u64::from(cells - 1))).unwrap(),
            read_write: source.bool(),
        },
        1 => Op::Child {
            pick: source.usize_in(0..=picks),
            vnode: u32::try_from(source.u64_in(1..=u64::from(vnodes))).unwrap(),
        },
        2 => Op::Cross {
            context: source.u64_in(1..=contexts),
            pick: source.usize_in(0..=picks),
            volume: u32::try_from(source.u64_in(1..=2)).unwrap(),
            budget: u32::try_from(source.u64_in(2..=budget_max)).unwrap(),
        },
        3 => Op::Pin {
            context: source.u64_in(1..=contexts),
            pick: source.usize_in(0..=picks),
            budget: u32::try_from(source.u64_in(2..=budget_max)).unwrap(),
        },
        4 => Op::Unpin {
            context: source.u64_in(1..=contexts),
            pick: source.usize_in(0..=picks),
        },
        5 => Op::Unbind {
            context: source.u64_in(1..=contexts),
            pick: source.usize_in(0..=picks),
        },
        _ => Op::Release {
            context: source.u64_in(1..=contexts),
        },
    }
}

/// What the model remembers.
#[derive(Default)]
struct Model {
    /// The largest id returned so far.
    largest: u64,
    /// Every node returned, with its file id if it has one.
    returned: BTreeMap<NodeId, Option<FileId>>,
    /// Pins per context and node.
    pins: BTreeMap<(u64, NodeId), u32>,
    /// Bindings per context and anchor.
    bindings: BTreeMap<(u64, NodeId), NodeId>,
}

impl Model {
    /// Records an id the table returned for `kind`.
    fn returned(&mut self, table: &NodeTable, node: NodeId) -> Result<(), String> {
        if !self.returned.contains_key(&node) {
            if node.0 <= self.largest {
                return Err(format!("{node:?} is not fresh (largest {})", self.largest));
            }
            self.largest = node.0;
        }
        let file = match table.kind(node) {
            Some(NodeKind::Root { file, .. } | NodeKind::Child { file, .. }) => Some(file),
            Some(_) => None,
            None => return Err(format!("{node:?} returned but not in the table")),
        };
        self.returned.insert(node, file);
        Ok(())
    }

    /// The nodes `node` names, from the table's public accessors.
    fn named(table: &NodeTable, node: NodeId) -> Vec<NodeId> {
        let mut named = Vec::new();
        match table.kind(node) {
            Some(NodeKind::Root { anchor, .. }) => named.push(anchor),
            Some(NodeKind::Entry { .. }) => named.push(DYNAMIC_ROOT),
            Some(NodeKind::Child { .. }) => {
                named.extend(table.parent(node));
                named.extend(table.containing_root(node));
            }
            Some(NodeKind::DynamicRoot) | None => {}
        }
        named
    }

    fn reach(table: &NodeTable, from: NodeId, into: &mut BTreeSet<NodeId>) {
        let mut stack = vec![from];
        while let Some(node) = stack.pop() {
            if into.insert(node) {
                stack.extend(Self::named(table, node));
            }
        }
    }

    fn expected_charge(&self, table: &NodeTable, ctx: u64) -> usize {
        let mut held = BTreeSet::new();
        for &(owner, node) in self.pins.keys() {
            if owner == ctx {
                Self::reach(table, node, &mut held);
            }
        }
        for (&(owner, _), &root) in &self.bindings {
            if owner == ctx {
                Self::reach(table, root, &mut held);
            }
        }
        held.len()
    }

    fn check(&self, table: &NodeTable, limit: u32, contexts: u64) -> Result<(), String> {
        if table.len() > limit as usize {
            return Err(format!("{} records over the limit {limit}", table.len()));
        }
        let live: Vec<NodeId> = self
            .returned
            .keys()
            .copied()
            .filter(|node| table.kind(*node).is_some())
            .collect();
        for node in &live {
            for named in Self::named(table, *node) {
                if table.kind(named).is_none() {
                    return Err(format!("{node:?} names {named:?}, which is gone"));
                }
            }
        }
        let files: BTreeSet<FileId> = self.returned.values().flatten().copied().collect();
        for file in files {
            let expected: Vec<NodeId> = live
                .iter()
                .copied()
                .filter(|node| self.returned.get(node) == Some(&Some(file)))
                .collect();
            let aliases = table.aliases(&file);
            if aliases != expected {
                return Err(format!(
                    "aliases of {file:?}: {aliases:?}, expected {expected:?}"
                ));
            }
        }
        for &(_, node) in self.pins.keys() {
            if table.kind(node).is_none() {
                return Err(format!("pinned {node:?} evicted"));
            }
        }
        for (&(ctx, from), &root) in &self.bindings {
            if table.binding(context(ctx), from).map(|(bound, _)| bound) != Some(root) {
                return Err(format!("binding of {from:?} in {ctx} lost"));
            }
            if table.kind(root).is_none() {
                return Err(format!("bound root {root:?} evicted"));
            }
        }
        for ctx in 1..=contexts {
            let expected = self.expected_charge(table, ctx);
            let charge = table.charge(context(ctx)) as usize;
            if charge != expected {
                return Err(format!(
                    "context {ctx} charged {charge}, expected {expected}"
                ));
            }
        }
        Ok(())
    }
}

/// The live nodes, in id order: what an index picks from.
fn live(table: &NodeTable, model: &Model) -> Vec<NodeId> {
    let mut live = vec![DYNAMIC_ROOT];
    live.extend(
        model
            .returned
            .keys()
            .copied()
            .filter(|node| table.kind(*node).is_some() && *node != DYNAMIC_ROOT),
    );
    live
}

fn pick(nodes: &[NodeId], index: usize) -> NodeId {
    nodes[index % nodes.len()]
}

/// The root a directory node serves from in `ctx`, and the parent a name
/// found in it gets: an anchor serves through its binding, a child through
/// its containing root.
fn directory(table: &NodeTable, ctx: u64, node: NodeId) -> Option<(NodeId, NodeId)> {
    match table.kind(node)? {
        NodeKind::Entry { .. } => table
            .binding(context(ctx), node)
            .map(|(root, _)| (node, root)),
        NodeKind::Child { .. } => match table.binding(context(ctx), node) {
            Some((root, _)) => Some((node, root)),
            None => table.containing_root(node).map(|root| (node, root)),
        },
        NodeKind::DynamicRoot | NodeKind::Root { .. } => None,
    }
}

/// What one operation did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Outcome {
    /// It took effect.
    Done,
    /// The table refused it.
    Refused,
    /// It could not be applied to the nodes picked.
    Skipped,
}

impl Op {
    /// The operation's index, for counting what a run exercised.
    fn index(self) -> usize {
        match self {
            Self::Entry { .. } => 0,
            Self::Child { .. } => 1,
            Self::Cross { .. } => 2,
            Self::Pin { .. } => 3,
            Self::Unpin { .. } => 4,
            Self::Unbind { .. } => 5,
            Self::Release { .. } => 6,
        }
    }
}

#[expect(
    clippy::too_many_lines,
    reason = "one arm per operation, each applying it to the table and the model"
)]
fn apply(
    table: &mut NodeTable,
    model: &mut Model,
    op: Op,
    nodes: &[NodeId],
    contexts: u64,
) -> Result<Outcome, String> {
    let outcome = |done: bool| {
        if done {
            Outcome::Done
        } else {
            Outcome::Refused
        }
    };
    Ok(match op {
        Op::Entry { cell, read_write } => {
            let path = if read_write {
                PathKind::ReadWrite
            } else {
                PathKind::ReadOnly
            };
            let made = table.entry(CellIndex(cell), path).ok();
            if let Some(node) = made {
                model.returned(table, node)?;
            }
            outcome(made.is_some())
        }
        Op::Child { pick: index, vnode } => {
            // Any context's binding will do to find a root to look in.
            let node = pick(nodes, index);
            let Some((parent, root)) = (1..=contexts).find_map(|ctx| directory(table, ctx, node))
            else {
                return Ok(Outcome::Skipped);
            };
            let made = table.child(parent, root, Vnode(vnode), Uniquifier(1)).ok();
            if let Some(child) = made {
                model.returned(table, child)?;
            }
            outcome(made.is_some())
        }
        Op::Cross {
            context: ctx,
            pick: index,
            volume,
            budget,
        } => {
            let from = pick(nodes, index);
            let cell = match table.kind(from) {
                Some(NodeKind::Entry { cell, .. }) => cell,
                Some(NodeKind::Child { file, .. }) => file.cell,
                _ => CellIndex(0),
            };
            let root = FileId {
                cell,
                volume: VolumeId(volume),
                vnode: Vnode(1),
                uniquifier: Uniquifier(1),
            };
            let bound = table
                .cross(context(ctx), budget, from, root, Version::ReadOnly)
                .ok();
            if let Some(bound) = bound {
                model.returned(table, bound)?;
                model.bindings.insert((ctx, from), bound);
            }
            outcome(bound.is_some())
        }
        Op::Pin {
            context: ctx,
            pick: index,
            budget,
        } => {
            let node = pick(nodes, index);
            let pinned = table.pin(context(ctx), budget, node).is_ok();
            if pinned {
                *model.pins.entry((ctx, node)).or_insert(0) += 1;
            }
            outcome(pinned)
        }
        Op::Unpin {
            context: ctx,
            pick: index,
        } => {
            let node = pick(nodes, index);
            let unpinned = table.unpin(context(ctx), node).is_ok();
            match model.pins.get_mut(&(ctx, node)) {
                Some(count) if unpinned => {
                    *count -= 1;
                    if *count == 0 {
                        model.pins.remove(&(ctx, node));
                    }
                }
                Some(_) => return Err("unpin of a pinned node refused".to_owned()),
                None if unpinned => return Err("unpin of an unpinned node done".to_owned()),
                None => {}
            }
            outcome(unpinned)
        }
        Op::Unbind {
            context: ctx,
            pick: index,
        } => {
            let from = pick(nodes, index);
            let had = model.bindings.remove(&(ctx, from)).is_some();
            if table.unbind(context(ctx), from) != had {
                return Err("unbind disagreed with the model".to_owned());
            }
            outcome(had)
        }
        Op::Release { context: ctx } => {
            table.release_context(context(ctx));
            model.pins.retain(|&(owner, _), _| owner != ctx);
            model.bindings.retain(|&(owner, _), _| owner != ctx);
            Outcome::Done
        }
    })
}

/// Runs `case` against the table and the model, counting in `done` the
/// operations of each kind that took effect.
fn run(case: &Case, contexts: u64, done: &mut [u64; 7]) -> Result<(), String> {
    let mut table = NodeTable::new(Limits {
        nodes: case.nodes,
        contexts: 4,
    })
    .map_err(|error| error.to_string())?;
    let mut model = Model::default();
    model.returned(&table, DYNAMIC_ROOT)?;
    for (step, &op) in case.ops.iter().enumerate() {
        let nodes = live(&table, &model);
        let charges = |table: &NodeTable| -> Vec<u32> {
            (1..=contexts)
                .map(|ctx| table.charge(context(ctx)))
                .collect()
        };
        let before = charges(&table);
        let len = table.len();
        let outcome = apply(&mut table, &mut model, op, &nodes, contexts)
            .map_err(|error| format!("step {step} {op:?}: {error}"))?;
        if outcome == Outcome::Done {
            done[op.index()] += 1;
        }
        if outcome != Outcome::Done && (charges(&table) != before || table.len() != len) {
            return Err(format!("step {step}: refused {op:?} changed the table"));
        }
        model
            .check(&table, case.nodes, contexts)
            .map_err(|error| format!("step {step} {op:?}: {error}"))?;
    }
    Ok(())
}

#[track_caller]
fn assert_every_operation_ran(done: &[u64; 7]) {
    assert!(
        done.iter().all(|&count| count > 0),
        "an operation never took effect: {done:?}"
    );
}

#[test]
fn node_table_matches_its_reference_model() {
    let mut done = [0; 7];
    Property::new("node table matches its reference model")
        .cases(512)
        .check(
            |source| Case {
                nodes: u32::try_from(source.u64_in(3..=12)).unwrap(),
                ops: source.vec_of(0..=48, |source| op(source, 15, 3, 2, 4, 8)),
            },
            |case| run(case, 3, &mut done),
        );
    assert_every_operation_ran(&done);
}

#[test]
fn node_table_matches_its_reference_model_on_every_short_sequence() {
    // Every sequence of three operations over two contexts, one cell, two
    // vnodes, four picks and budgets of 2 and 3, in a table of four
    // records; the sweep prints how many it ran and the bound.
    let mut done = [0; 7];
    Exhaustive::new("node table, every three-operation sequence", 500_000).check(
        |source| Case {
            nodes: 4,
            ops: (0..3).map(|_| op(source, 3, 2, 1, 2, 3)).collect(),
        },
        |case| run(case, 2, &mut done),
    );
    assert_every_operation_ran(&done);
}
