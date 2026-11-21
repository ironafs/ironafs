// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for merging a node's pending effects, and for when they widen
//! (ADR 0004, C17, point 6; N47, point 3).

use super::{Effects, Entries, Rights, Scope};
use crate::{ContextId, Name};

fn name(bytes: &[u8]) -> Name {
    Name::new(bytes).unwrap()
}

fn entries(entries: Entries) -> Effects {
    Effects {
        entries: Some(entries),
        ..Effects::NONE
    }
}

fn rights(rights: Rights) -> Effects {
    Effects {
        rights: Some(rights),
        ..Effects::NONE
    }
}

#[test]
fn merge_keeps_data_and_attributes_from_either_side() {
    let mut slot = Effects {
        data: true,
        ..Effects::NONE
    };
    slot.merge(Effects {
        attributes: true,
        ..Effects::NONE
    });
    assert!(slot.data);
    assert!(slot.attributes);
}

#[test]
fn merge_into_an_empty_slot_takes_the_event() {
    let mut slot = Effects::NONE;
    slot.merge(entries(Entries::Name(name(b"a"))));
    assert_eq!(slot, entries(Entries::Name(name(b"a"))));
}

#[test]
fn merge_of_the_same_entry_name_keeps_it() {
    let mut slot = entries(Entries::Name(name(b"a")));
    slot.merge(entries(Entries::Name(name(b"a"))));
    assert_eq!(slot.entries, Some(Entries::Name(name(b"a"))));
}

#[test]
fn merge_of_different_entry_names_widens_to_all_entries() {
    let mut slot = entries(Entries::Name(name(b"a")));
    slot.merge(entries(Entries::Name(name(b"b"))));
    assert_eq!(slot.entries, Some(Entries::All));
}

#[test]
fn merge_of_a_rename_with_another_name_widens_to_all_entries() {
    let rename = Entries::Rename {
        from: name(b"a"),
        to: name(b"b"),
    };
    let mut slot = entries(rename.clone());
    slot.merge(entries(Entries::Name(name(b"a"))));
    assert_eq!(slot.entries, Some(Entries::All));

    let mut slot = entries(rename.clone());
    slot.merge(entries(rename.clone()));
    assert_eq!(slot.entries, Some(rename));
}

#[test]
fn merge_into_all_entries_stays_all_entries() {
    let mut slot = entries(Entries::All);
    slot.merge(entries(Entries::Name(name(b"a"))));
    assert_eq!(slot.entries, Some(Entries::All));
}

#[test]
fn merge_of_rights_for_one_context_keeps_it() {
    let mut slot = rights(Rights::Context(ContextId(3)));
    slot.merge(rights(Rights::Context(ContextId(3))));
    assert_eq!(slot.rights, Some(Rights::Context(ContextId(3))));
}

#[test]
fn merge_of_rights_for_different_contexts_widens_to_all_contexts() {
    let mut slot = rights(Rights::Context(ContextId(3)));
    slot.merge(rights(Rights::Context(ContextId(4))));
    assert_eq!(slot.rights, Some(Rights::All));
}

#[test]
fn rights_and_entry_events_on_one_node_both_stay_pending() {
    let mut slot = rights(Rights::Context(ContextId(3)));
    slot.merge(entries(Entries::Name(name(b"a"))));
    assert_eq!(slot.rights, Some(Rights::Context(ContextId(3))));
    assert_eq!(slot.entries, Some(Entries::Name(name(b"a"))));
}

#[test]
fn covers_is_true_for_a_wider_slot_and_false_for_a_narrower_one() {
    let wide = Effects {
        data: true,
        attributes: true,
        entries: Some(Entries::All),
        rights: Some(Rights::All),
    };
    let narrow = Effects {
        data: true,
        attributes: false,
        entries: Some(Entries::Name(name(b"a"))),
        rights: Some(Rights::Context(ContextId(1))),
    };
    assert!(wide.covers(&narrow));
    assert!(!narrow.covers(&wide));
    assert!(narrow.covers(&Effects::NONE));
    assert!(!Effects::NONE.covers(&narrow));
}

#[test]
fn scope_orders_from_object_to_server() {
    assert!(Scope::Object < Scope::Directory);
    assert!(Scope::Directory < Scope::Volume);
    assert!(Scope::Volume < Scope::Server);
}

#[test]
fn covers_counts_both_names_of_a_rename() {
    let rename = entries(Entries::Rename {
        from: name(b"a"),
        to: name(b"b"),
    });
    assert!(rename.covers(&entries(Entries::Name(name(b"a")))));
    assert!(rename.covers(&entries(Entries::Name(name(b"b")))));
    assert!(!rename.covers(&entries(Entries::Name(name(b"c")))));
}

#[test]
fn covers_takes_a_rename_in_either_direction_as_the_same_names() {
    let forward = entries(Entries::Rename {
        from: name(b"a"),
        to: name(b"b"),
    });
    let back = entries(Entries::Rename {
        from: name(b"b"),
        to: name(b"a"),
    });
    assert!(forward.covers(&back));
    assert!(back.covers(&forward));
}

#[test]
fn one_name_never_covers_a_rename() {
    let rename = entries(Entries::Rename {
        from: name(b"a"),
        to: name(b"a2"),
    });
    assert!(!entries(Entries::Name(name(b"a"))).covers(&rename));
    assert!(!entries(Entries::Name(name(b"a2"))).covers(&rename));
}
