// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Properties of the cell name: a name the rule allows is accepted in its
//! canonical form, whatever its case and final dot; every accepted name
//! has the rule's shape and constructs itself again.

use ironafs_config::CellName;
use ironafs_test_support::{Property, Source};

const LETTERS: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";

/// A label the rule allows: letters and digits at both ends, hyphens
/// allowed inside.
fn label(source: &mut Source) -> Vec<u8> {
    let length = source.usize_in(1..=63);
    (0..length)
        .map(|index| {
            if index == 0 || index == length - 1 || source.weighted(&[4, 1]) == 0 {
                *source.pick(LETTERS)
            } else {
                b'-'
            }
        })
        .collect()
}

/// A name the rule allows, as typed: any case, perhaps a final dot.
fn valid_input(source: &mut Source) -> Vec<u8> {
    let mut name = label(source);
    for _ in 0..source.usize_in(0..=4) {
        let next = label(source);
        if name.len() + 1 + next.len() > 253 {
            break;
        }
        name.push(b'.');
        name.extend(next);
    }
    if source.bool() {
        name.push(b'.');
    }
    name
}

/// The rule's shape, checked directly on a constructed name.
fn has_rule_shape(name: &str) -> Result<(), String> {
    if name.is_empty() || name.len() > 253 {
        return Err(format!("length {}", name.len()));
    }
    for label in name.split('.') {
        let bytes = label.as_bytes();
        if bytes.is_empty() || bytes.len() > 63 {
            return Err(format!("label {label:?}"));
        }
        if bytes.first() == Some(&b'-') || bytes.last() == Some(&b'-') {
            return Err(format!("edge hyphen in {label:?}"));
        }
        if !bytes
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
        {
            return Err(format!("octet in {label:?}"));
        }
    }
    Ok(())
}

#[test]
fn new_accepts_every_valid_name_in_canonical_form() {
    Property::new("cell name: valid input").check(valid_input, |input| {
        let name = CellName::new(input).map_err(|error| error.to_string())?;
        let mut want = input.to_ascii_lowercase();
        if want.last() == Some(&b'.') {
            want.pop();
        }
        if name.as_str().as_bytes() != want.as_slice() {
            return Err(format!("stored {:?}", name.as_str()));
        }
        Ok(())
    });
}

#[test]
fn new_accepted_names_have_rule_shape_and_are_fixed_points() {
    const PIECES: &[&[u8]] = &[
        b"a",
        b"Z",
        b"0",
        b"-",
        b".",
        b"..",
        b"_",
        b"\xC3\xA9",
        b" ",
        b"\xFF",
        b"xn--",
    ];
    Property::new("cell name: any input").cases(4_096).check(
        |source| {
            let mut bytes = Vec::new();
            for _ in 0..source.usize_in(0..=80) {
                bytes.extend_from_slice(source.pick(PIECES));
            }
            bytes
        },
        |input| {
            let upper = CellName::new(&input.to_ascii_uppercase());
            let Ok(name) = CellName::new(input) else {
                // Case never decides whether a name is refused.
                return match upper {
                    Ok(_) => Err("refused, but accepted upper-cased".to_string()),
                    Err(_) => Ok(()),
                };
            };
            has_rule_shape(name.as_str())?;
            if CellName::new(name.as_str().as_bytes()).as_ref() != Ok(&name) {
                return Err("not a fixed point".to_string());
            }
            if upper.as_ref() != Ok(&name) {
                return Err("upper case gives another name".to_string());
            }
            Ok(())
        },
    );
}
