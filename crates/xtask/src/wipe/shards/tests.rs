// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Synthetic shard filenames, symbols, aliases and boundaries.

use super::{Symbols, read_assembly};
use crate::{Error, fixture};

const PLAIN: &str = "_RNvCs9_17ironafs_wipe_gate10plain_dead";
const UNTOUCHED: &str = "_RNvCs9_17ironafs_wipe_gate14untouched_dead";

#[test]
fn alias_uses_the_defining_shard_of_its_exact_target() {
    let first = format!("{UNTOUCHED} = {PLAIN}\n");
    let second = format!("{PLAIN}:\n\tretq\n.Lfunc_end0:\n");
    let symbols = Symbols::index([first.as_str(), second.as_str()]).unwrap();
    assert_eq!(symbols.body("untouched_dead").unwrap(), ["\tretq"]);
}

#[test]
fn duplicate_alias_or_defining_label_across_shards_fails() {
    let alias = format!("{UNTOUCHED} = {PLAIN}\n");
    let body = format!("{PLAIN}:\n\tretq\n.Lfunc_end0:\n");
    for shard in [alias.as_str(), body.as_str()] {
        assert!(matches!(
            Symbols::index([shard, shard]),
            Err("duplicate assembly symbol definition")
        ));
    }
    let collision = format!("{UNTOUCHED}:\n\tretq\n.Lfunc_end0:\n");
    assert!(matches!(
        Symbols::index([alias.as_str(), collision.as_str()]),
        Err("duplicate assembly symbol definition")
    ));
}

#[test]
fn cyclic_or_unresolved_alias_fails() {
    let one = format!("{UNTOUCHED} = {PLAIN}\n");
    let two = format!("{PLAIN} = {UNTOUCHED}\n");
    let symbols = Symbols::index([one.as_str(), two.as_str()]).unwrap();
    assert_eq!(symbols.body("untouched_dead"), Err("cyclic assembly alias"));
    let symbols = Symbols::index([one.as_str()]).unwrap();
    assert_eq!(
        symbols.body("untouched_dead"),
        Err("unresolved assembly alias target")
    );
}

#[test]
fn body_without_a_function_boundary_does_not_qualify() {
    let shard = format!("{PLAIN}:\n\tretq\n");
    let symbols = Symbols::index([shard.as_str()]).unwrap();
    assert_eq!(
        symbols.body("plain_dead"),
        Err("assembly function has no boundary")
    );
}

#[test]
fn assembly_uses_only_the_recorded_prefix_in_the_recorded_output_directory() {
    let root = fixture::fresh_dir(&std::env::temp_dir(), "wipe-assembly-shards").unwrap();
    let dir = root.join("emitting");
    let stem = "ironafs_wipe_gate-current";
    fixture::write_file(&dir.join("ironafs_wipe_gate-old.s"), "stale").unwrap();
    fixture::write_file(&dir.join("ironafs_wipe_gate-current-foreign.s"), "foreign").unwrap();
    fixture::write_file(&dir.join("nested").join(format!("{stem}.s")), "nested").unwrap();
    fixture::write_file(
        &root.join("other-build").join(format!("{stem}.s")),
        "stale exact name",
    )
    .unwrap();

    let Error::Rules { violations, .. } = read_assembly(&dir, stem).unwrap_err() else {
        panic!("expected missing assembly shards")
    };
    assert_eq!(
        violations,
        ["no assembly shards for the captured harness invocation"]
    );

    fixture::write_file(&dir.join(format!("{stem}.a.rcgu.s")), "shard a").unwrap();
    fixture::write_file(&dir.join(format!("{stem}.b.rcgu.s")), "shard b").unwrap();
    assert_eq!(read_assembly(&dir, stem).unwrap(), ["shard a", "shard b"]);
    fixture::remove_dir(&root).unwrap();
}
