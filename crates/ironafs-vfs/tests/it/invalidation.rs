// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Merging pending effects never drops one (ADR 0004, C17, point 6), over
//! random pairs of effects.

use ironafs_test_support::{Property, Source};
use ironafs_vfs::{ContextId, Effects, Entries, Name, Rights};

fn name(source: &mut Source) -> Name {
    Name::new(source.pick(&[&b"a"[..], b"b", b"c"])).unwrap()
}

fn effects(source: &mut Source) -> Effects {
    let entries = match source.u64_in(0..=3) {
        0 => None,
        1 => Some(Entries::Name(name(source))),
        2 => Some(Entries::Rename {
            from: name(source),
            to: name(source),
        }),
        _ => Some(Entries::All),
    };
    let rights = match source.u64_in(0..=2) {
        0 => None,
        1 => Some(Rights::Context(ContextId(source.u64_in(1..=3)))),
        _ => Some(Rights::All),
    };
    Effects {
        data: source.bool(),
        attributes: source.bool(),
        entries,
        rights,
    }
}

#[test]
fn merged_effects_cover_both_sides() {
    Property::new("merged effects cover both sides").check(
        |source: &mut Source| (effects(source), effects(source)),
        |(ours, theirs)| {
            let mut merged = ours.clone();
            merged.merge(theirs.clone());
            assert!(merged.covers(ours), "{merged:?} does not cover {ours:?}");
            assert!(
                merged.covers(theirs),
                "{merged:?} does not cover {theirs:?}"
            );
        },
    );
}
