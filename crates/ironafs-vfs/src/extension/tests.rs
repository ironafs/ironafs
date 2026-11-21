// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for the bounds of an extension list and for finding the
//! extensions a receiver does not know.

use alloc::vec;
use alloc::vec::Vec;

use super::{Extension, ExtensionError, Extensions, MAX_EXTENSION_BYTES, MAX_EXTENSIONS};

fn plain(tag: u32) -> Extension {
    Extension::new(tag, false, vec![1, 2, 3]).unwrap()
}

#[test]
fn extension_keeps_its_tag_flag_and_bytes() {
    let extension = Extension::new(9, true, vec![4, 5]).unwrap();
    assert_eq!(extension.tag(), 9);
    assert!(extension.critical());
    assert_eq!(extension.bytes(), &[4, 5]);
}

#[test]
fn extension_caps_are_16_extensions_and_4096_bytes() {
    assert_eq!(MAX_EXTENSIONS, 16);
    assert_eq!(MAX_EXTENSION_BYTES, 4096);
}

#[test]
fn extension_over_the_byte_cap_is_refused() {
    let bytes = vec![0; 4097];
    assert_eq!(
        Extension::new(1, false, bytes).unwrap_err(),
        ExtensionError::TooLarge
    );
}

#[test]
fn extension_at_the_byte_cap_is_accepted() {
    let bytes = vec![0; 4096];
    assert_eq!(Extension::new(1, false, bytes).unwrap().bytes().len(), 4096);
}

#[test]
fn list_refuses_more_than_its_count_cap() {
    let mut list = Extensions::new();
    for tag in 0..16 {
        list.push(plain(tag)).unwrap();
    }
    assert_eq!(
        list.push(plain(1_000)).unwrap_err(),
        ExtensionError::TooMany
    );
    assert_eq!(list.iter().count(), 16);
}

#[test]
fn list_refuses_more_than_its_byte_cap_in_total() {
    let mut list = Extensions::new();
    list.push(Extension::new(1, false, vec![0; 2048]).unwrap())
        .unwrap();
    list.push(Extension::new(2, false, vec![0; 2048]).unwrap())
        .unwrap();
    let over = Extension::new(3, false, vec![0]).unwrap();
    assert_eq!(list.push(over).unwrap_err(), ExtensionError::TooLarge);
    assert_eq!(list.iter().count(), 2);
}

#[test]
fn list_refuses_a_second_extension_with_the_same_tag() {
    let mut list = Extensions::new();
    list.push(plain(4)).unwrap();
    assert_eq!(list.push(plain(4)).unwrap_err(), ExtensionError::Duplicate);
}

#[test]
fn unknown_critical_extension_is_found() {
    let mut list = Extensions::new();
    list.push(plain(1)).unwrap();
    list.push(Extension::new(2, true, Vec::new()).unwrap())
        .unwrap();
    list.push(Extension::new(3, true, Vec::new()).unwrap())
        .unwrap();
    assert_eq!(list.first_unknown_critical(&[1, 2]), Some(3));
}

#[test]
fn unknown_plain_and_known_critical_extensions_are_not_refusals() {
    let mut list = Extensions::new();
    list.push(plain(1)).unwrap();
    list.push(Extension::new(2, true, Vec::new()).unwrap())
        .unwrap();
    assert_eq!(list.first_unknown_critical(&[2]), None);
    assert_eq!(Extensions::new().first_unknown_critical(&[]), None);
}

#[test]
fn unknown_tags_names_every_extension_the_receiver_does_not_know() {
    let mut list = Extensions::new();
    for tag in [5, 6, 7] {
        list.push(plain(tag)).unwrap();
    }
    assert_eq!(list.unknown_tags(&[6]).collect::<Vec<_>>(), vec![5, 7]);
}
