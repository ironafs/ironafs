// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Properties of request decoding over seeded random input
//! (fuse-codec.md 10.2): it never panics, and what it accepts it reports
//! faithfully.

use ironafs_fuse::request::{DecodeError, Header, Phase, Request, decode};
use ironafs_test_support::{Property, Source};

fn phase(source: &mut Source) -> Phase {
    if source.bool() {
        Phase::AwaitingInit
    } else {
        Phase::Negotiated {
            minor: u32::try_from(source.u64_in(23..=46)).unwrap(),
        }
    }
}

#[test]
fn decode_never_panics_on_any_bytes() {
    Property::new("decode returns on any bytes")
        .cases(4096)
        .check(
            |source| (source.bytes(0..=200), phase(source)),
            |(bytes, phase)| {
                let _ = decode(bytes, *phase);
            },
        );
}

/// A request with a well-formed header around a random body.
#[derive(Debug)]
struct Framed {
    header: Header,
    extlen: u16,
    body: Vec<u8>,
    phase: Phase,
}

impl Framed {
    fn generate(source: &mut Source) -> Self {
        let opcode = if source.bool() {
            u32::try_from(source.u64_in(0..=60)).unwrap()
        } else {
            source.u32()
        };
        Self {
            header: Header {
                opcode,
                unique: source.u64(),
                nodeid: source.u64(),
                uid: source.u32(),
                gid: source.u32(),
                pid: source.u32(),
            },
            extlen: if source.bool() { 0 } else { source.u16() },
            body: source.bytes(0..=300),
            phase: phase(source),
        }
    }

    fn bytes(&self) -> Vec<u8> {
        let len = u32::try_from(40 + self.body.len()).unwrap();
        [
            &len.to_le_bytes()[..],
            &self.header.opcode.to_le_bytes(),
            &self.header.unique.to_le_bytes(),
            &self.header.nodeid.to_le_bytes(),
            &self.header.uid.to_le_bytes(),
            &self.header.gid.to_le_bytes(),
            &self.header.pid.to_le_bytes(),
            &self.extlen.to_le_bytes(),
            &[0, 0],
            &self.body,
        ]
        .concat()
    }
}

#[test]
fn decode_reports_framed_requests_faithfully() {
    Property::new("decode keeps the header and the name of a framed request")
        .cases(4096)
        .check(Framed::generate, |framed| {
            let bytes = framed.bytes();
            match decode(&bytes, framed.phase) {
                Ok((header, request)) => {
                    assert_eq!(header, framed.header);
                    if let Request::Lookup { name, reply } = request {
                        assert_eq!(reply.unique(), framed.header.unique);
                        assert_eq!(
                            Some(name.as_bytes()),
                            framed.body.split_last().map(|(_, name)| name)
                        );
                    }
                }
                Err(DecodeError::Request(error)) => assert_eq!(error.header, framed.header),
                Err(DecodeError::Frame(error)) => panic!("a framed request: {error:?}"),
            }
        });
}
