// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Test datagrams, written from the layout of
//! docs/specs/network-generation.md, 5.4, in the host's byte order: shared
//! by the decoder's and the monitor's tests. Independent of the decoder:
//! it shares no code with it.

use alloc::vec::Vec;

/// `AF_INET` and `AF_INET6`.
pub(crate) const AF_INET: u8 = 2;
pub(crate) const AF_INET6: u8 = 10;

/// One message: a 16-byte header of length `16 + body.len()` (unpadded),
/// then `body`, then padding to 4 bytes.
pub(crate) fn message(kind: u16, flags: u16, sequence: u32, body: &[u8]) -> Vec<u8> {
    let length = u32::try_from(16 + body.len()).unwrap();
    let mut bytes = Vec::new();
    bytes.extend(length.to_ne_bytes());
    bytes.extend(kind.to_ne_bytes());
    bytes.extend(flags.to_ne_bytes());
    bytes.extend(sequence.to_ne_bytes());
    bytes.extend(0_u32.to_ne_bytes());
    bytes.extend(body);
    // Padding only between messages: a lone message keeps its length, and
    // `pad` adds it where one follows.
    bytes
}

/// `bytes` padded with zeros to a multiple of 4.
pub(crate) fn pad(mut bytes: Vec<u8>) -> Vec<u8> {
    while !bytes.len().is_multiple_of(4) {
        bytes.push(0);
    }
    bytes
}

/// Appends one attribute (header, value, padding) to `body`.
pub(crate) fn attribute(body: &mut Vec<u8>, kind: u16, value: &[u8]) {
    let length = u16::try_from(4 + value.len()).unwrap();
    body.extend(length.to_ne_bytes());
    body.extend(kind.to_ne_bytes());
    body.extend(value);
    while !body.len().is_multiple_of(4) {
        body.push(0);
    }
}

/// A link message of `kind` for link `index` with `flags`, its
/// attributes appended by `attributes`, padded.
pub(crate) fn link(
    kind: u16,
    index: i32,
    flags: u32,
    attributes: impl FnOnce(&mut Vec<u8>),
) -> Vec<u8> {
    link_with(kind, 0, index, flags, attributes)
}

/// As [`link`], with a sequence number.
pub(crate) fn link_with(
    kind: u16,
    sequence: u32,
    index: i32,
    flags: u32,
    attributes: impl FnOnce(&mut Vec<u8>),
) -> Vec<u8> {
    let mut body = Vec::new();
    body.extend([0, 0, 1, 0]);
    body.extend(index.to_ne_bytes());
    body.extend(flags.to_ne_bytes());
    body.extend(0_u32.to_ne_bytes());
    attributes(&mut body);
    pad(message(kind, 0, sequence, &body))
}

/// An address message of `kind`, padded.
pub(crate) fn address(
    kind: u16,
    family: u8,
    index: u32,
    prefix_length: u8,
    flags: u8,
    attributes: impl FnOnce(&mut Vec<u8>),
) -> Vec<u8> {
    address_with(kind, 0, family, index, prefix_length, flags, attributes)
}

/// As [`address`], with a sequence number.
pub(crate) fn address_with(
    kind: u16,
    sequence: u32,
    family: u8,
    index: u32,
    prefix_length: u8,
    flags: u8,
    attributes: impl FnOnce(&mut Vec<u8>),
) -> Vec<u8> {
    let mut body = Vec::new();
    body.extend([family, prefix_length, flags, 0]);
    body.extend(index.to_ne_bytes());
    attributes(&mut body);
    pad(message(kind, 0, sequence, &body))
}
