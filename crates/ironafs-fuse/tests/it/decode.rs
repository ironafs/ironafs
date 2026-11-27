// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Properties of request decoding over seeded random input
//! (fuse-codec.md 10.2): it never panics, and what it accepts it reports
//! faithfully.

use ironafs_fuse::init::InitConfig;
use ironafs_fuse::request::{DecodeError, Header, Request};
use ironafs_fuse::session::Session;
use ironafs_test_support::{Property, Source};

/// Answers and finishes an INIT at 7.`minor`, through the public API.
fn settle(session: &mut Session, minor: u32) {
    let body = [7u32, minor, 4096, 0].map(u32::to_le_bytes).concat();
    let init = [
        &56u32.to_le_bytes()[..],
        &26u32.to_le_bytes(),
        &[0; 32],
        &body,
    ]
    .concat();
    let Ok((_, Request::Init { reply })) = session.decode(&init) else {
        panic!("init refused");
    };
    let config = InitConfig::new(4096, 131_072, 12, 9, 1).unwrap();
    session.finish(reply.answer(&config).0).unwrap();
    assert_eq!(session.minor(), Some(minor));
}

#[test]
fn decode_never_panics_on_any_bytes() {
    Property::new("decode returns on any bytes")
        .cases(4096)
        .check(
            |source| (source.bytes(0..=200), source.bool()),
            |(bytes, negotiated)| {
                let mut session = Session::new();
                if *negotiated {
                    settle(&mut session, 40);
                }
                let _ = session.decode(bytes);
            },
        );
}

/// A request with a well-formed header around a random body.
#[derive(Debug)]
struct Framed {
    header: Header,
    extlen: u16,
    body: Vec<u8>,
    /// The minor the session is at, or 0 before INIT.
    minor: u32,
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
            minor: u32::try_from(source.u64_in(0..=46)).unwrap(),
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
            let mut session = Session::new();
            if framed.minor >= 23 {
                settle(&mut session, framed.minor);
            }
            match session.decode(&bytes) {
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
