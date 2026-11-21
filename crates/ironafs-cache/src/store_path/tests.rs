// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The refused names of the seams and scheduling specification, 10.3, and
//! the names that are accepted.

use alloc::string::String;
use alloc::vec::Vec;

use super::{MAX_COMPONENTS, MAX_NAME_BYTES, StoreDir, StoreName, StorePath};

#[test]
fn store_name_refuses_each_bad_name() {
    let too_long: String = "a".repeat(MAX_NAME_BYTES + 1);
    for refused in [
        "",
        ".",
        "..",
        "Upper",
        "a b",
        "a/b",
        "a\\b",
        "caf\u{e9}",
        "a:b",
        "a\0b",
        too_long.as_str(),
    ] {
        assert_eq!(StoreName::new(refused), None, "{refused:?}");
    }
}

#[test]
fn store_name_accepts_the_restricted_set() {
    let longest: String = "z".repeat(MAX_NAME_BYTES);
    for accepted in [
        "a",
        "0",
        "tmp.1",
        "format-marker",
        "a_b",
        "...",
        longest.as_str(),
    ] {
        assert_eq!(
            StoreName::new(accepted).map(|name| String::from(name.as_str())),
            Some(String::from(accepted))
        );
    }
}

#[test]
fn store_dir_refuses_a_bad_component() {
    for refused in ["", "a/", "/a", "a//b", "a/./b", "a/../b", "a/B", ".."] {
        assert_eq!(StoreDir::parse(refused), None, "{refused:?}");
    }
}

#[test]
fn store_dir_keeps_its_components_in_order() {
    let dir = StoreDir::parse("clean/0f3a").unwrap();
    let names: Vec<&str> = dir.names().iter().map(StoreName::as_str).collect();
    assert_eq!(names, ["clean", "0f3a"]);
}

#[test]
fn store_path_refuses_one_component_too_many() {
    let deepest = ["d"; MAX_COMPONENTS - 1].join("/");
    let too_deep = ["d"; MAX_COMPONENTS].join("/");
    let name = StoreName::new("f").unwrap();
    let path = StorePath::new(StoreDir::parse(&deepest).unwrap(), name.clone()).unwrap();
    assert_eq!(path.dir().names().len(), MAX_COMPONENTS - 1);
    assert_eq!(path.name().as_str(), "f");
    // A directory as deep as a whole path leaves no room for the name.
    assert_eq!(
        StoreDir::parse(&too_deep).and_then(|dir| StorePath::new(dir, name)),
        None
    );
    assert_eq!(StoreDir::parse(&[too_deep.as_str(), "d"].join("/")), None);
}
