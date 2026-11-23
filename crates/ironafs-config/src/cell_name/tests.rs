// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

use alloc::format;
use alloc::string::{String, ToString};

use super::*;

#[track_caller]
fn name(input: &str) -> String {
    CellName::new(input.as_bytes())
        .unwrap()
        .as_str()
        .to_string()
}

#[track_caller]
fn refused(input: &[u8]) -> CellNameError {
    CellName::new(input).unwrap_err()
}

/// A name of exactly `length` bytes: labels of 63 bytes joined by dots,
/// the last one shorter.
fn name_of_length(length: usize) -> String {
    let mut text = String::new();
    while text.len() < length {
        if !text.is_empty() {
            text.push('.');
        }
        let label = (length - text.len()).min(63);
        text.push_str(&"a".repeat(label));
    }
    text
}

#[test]
fn new_accepts_253_bytes() {
    let input = name_of_length(253);
    assert_eq!(input.len(), 253);
    assert_eq!(name(&input), input);
}

#[test]
fn new_refuses_254_bytes_without_final_dot() {
    let input = name_of_length(254);
    assert_eq!(input.len(), 254);
    assert_eq!(
        refused(input.as_bytes()),
        CellNameError::TooLong { offset: 253 }
    );
}

#[test]
fn new_strips_one_final_dot_from_254_bytes() {
    let stored = name_of_length(253);
    let input = format!("{stored}.");
    assert_eq!(input.len(), 254);
    assert_eq!(name(&input), stored);
}

#[test]
fn new_refuses_255_bytes_ending_in_one_dot() {
    let input = format!("{}.", name_of_length(254));
    assert_eq!(
        refused(input.as_bytes()),
        CellNameError::TooLong { offset: 253 }
    );
}

#[test]
fn new_accepts_63_byte_label_and_refuses_64() {
    let label = "b".repeat(63);
    assert_eq!(name(&format!("{label}.org")), format!("{label}.org"));
    assert_eq!(
        refused(format!("{label}b.org").as_bytes()),
        CellNameError::LabelTooLong { offset: 0 }
    );
    assert_eq!(
        refused(format!("org.{label}b").as_bytes()),
        CellNameError::LabelTooLong { offset: 4 }
    );
}

#[test]
fn new_refuses_underscore() {
    assert_eq!(
        refused(b"_afs.example.org"),
        CellNameError::Underscore { offset: 0 }
    );
    assert_eq!(
        refused(b"ex_ample.org"),
        CellNameError::Underscore { offset: 2 }
    );
}

#[test]
fn new_refuses_two_final_dots() {
    assert_eq!(
        refused(b"example.org.."),
        CellNameError::TwoFinalDots { offset: 11 }
    );
    assert_eq!(refused(b".."), CellNameError::TwoFinalDots { offset: 0 });
}

#[test]
fn new_refuses_empty_name() {
    assert_eq!(refused(b""), CellNameError::Empty { offset: 0 });
    assert_eq!(refused(b"."), CellNameError::Empty { offset: 0 });
}

#[test]
fn new_stores_mixed_case_lower_cased() {
    assert_eq!(name("Example.ORG"), "example.org");
    assert_eq!(
        CellName::new(b"Example.ORG").unwrap(),
        CellName::new(b"example.org.").unwrap()
    );
}

#[test]
fn new_refuses_empty_labels() {
    assert_eq!(
        refused(b".example.org"),
        CellNameError::EmptyLabel { offset: 0 }
    );
    assert_eq!(refused(b"a..b"), CellNameError::EmptyLabel { offset: 2 });
}

#[test]
fn new_accepts_inner_hyphens_only() {
    assert_eq!(name("my-cell.example-site.org"), "my-cell.example-site.org");
    assert_eq!(
        refused(b"-a.org"),
        CellNameError::HyphenAtLabelEdge { offset: 0 }
    );
    assert_eq!(
        refused(b"a-.org"),
        CellNameError::HyphenAtLabelEdge { offset: 1 }
    );
    assert_eq!(
        refused(b"a.-b"),
        CellNameError::HyphenAtLabelEdge { offset: 2 }
    );
    assert_eq!(
        refused(b"-"),
        CellNameError::HyphenAtLabelEdge { offset: 0 }
    );
}

#[test]
fn new_accepts_idn_in_ascii_form_without_validating_it() {
    assert_eq!(name("xn--bcher-kva.example"), "xn--bcher-kva.example");
    assert_eq!(name("XN--ZZZZ.example"), "xn--zzzz.example");
}

#[test]
fn new_refuses_non_ascii() {
    assert_eq!(
        refused("caf\u{e9}.org".as_bytes()),
        CellNameError::NotAscii { offset: 3 }
    );
    assert_eq!(refused(b"a\xFF"), CellNameError::NotAscii { offset: 1 });
}

#[test]
fn new_refuses_other_octets() {
    assert_eq!(
        refused(b"a b.org"),
        CellNameError::InvalidOctet {
            offset: 1,
            octet: b' '
        }
    );
    assert_eq!(
        refused(b"a/b"),
        CellNameError::InvalidOctet {
            offset: 1,
            octet: b'/'
        }
    );
    assert_eq!(
        refused(b"ab\0"),
        CellNameError::InvalidOctet {
            offset: 2,
            octet: 0
        }
    );
}

#[test]
fn new_accepts_single_label_and_digits() {
    assert_eq!(name("localcell"), "localcell");
    assert_eq!(name("1.2.3.4"), "1.2.3.4");
}

#[test]
fn new_checks_length_before_labels() {
    assert_eq!(
        refused(&[b'_'; 254]),
        CellNameError::TooLong { offset: 253 }
    );
    assert_eq!(
        refused(&[b'_'; 64]),
        CellNameError::LabelTooLong { offset: 0 }
    );
}

#[test]
fn error_messages_name_whole_name_offsets() {
    assert_eq!(
        CellNameError::Empty { offset: 0 }.to_string(),
        "cell name is empty at byte 0"
    );
    assert_eq!(
        CellNameError::TwoFinalDots { offset: 11 }.to_string(),
        "cell name ends with two dots at byte 11"
    );
}

#[test]
fn display_is_the_name() {
    assert_eq!(
        CellName::new(b"Example.Org").unwrap().to_string(),
        "example.org"
    );
}

#[test]
fn error_messages_name_the_offset() {
    assert_eq!(
        CellNameError::Underscore { offset: 2 }.to_string(),
        "cell name has an underscore at byte 2"
    );
    assert_eq!(
        CellNameError::TooLong { offset: 253 }.to_string(),
        "cell name is longer than 253 bytes: byte 253 is over the limit"
    );
}
