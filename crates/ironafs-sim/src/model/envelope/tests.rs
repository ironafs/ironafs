// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The envelope round-trips every request and answer, and rejects every
//! truncation and any trailing byte.

use super::{EnvelopeError, decode_answer, decode_request, encode_answer, encode_request};
use crate::model::callback::{CbProc, Reset};
use crate::model::types::{
    Addr, Answer, ClientId, ContextId, Fid, FileKind, Procedure, Promise, Release, Request,
    Results, ServerUuid, Service, Site, SiteServer, Status, VlEntry, VolSync, VolumeId, VolumeIds,
    VolumeKind, VolumeStatus,
};

fn requests() -> Vec<Request> {
    let fid = Fid::new(536_870_915, 2, 4_000_000_000);
    let procedures = vec![
        Procedure::VlGetEntryByName {
            name: b"root.cell".to_vec(),
        },
        Procedure::VlGetEntryById {
            volume: VolumeId(7),
            kind: VolumeKind::Backup,
        },
        Procedure::VlGetAddrs {
            server: ServerUuid([0x80; 16]),
        },
        Procedure::FsFetchStatus { fid },
        Procedure::FsBulkStatus {
            fids: vec![fid, Fid::new(1, 3, 5)],
        },
        Procedure::FsInlineBulkStatus { fids: vec![fid] },
        Procedure::FsFetchData {
            fid,
            offset: u32::MAX,
            length: 16,
        },
        Procedure::FsFetchData64 {
            fid,
            offset: u64::MAX - 1,
            length: 1 << 32,
        },
        Procedure::FsGetVolumeStatus {
            volume: VolumeId(9),
        },
        Procedure::PeerWrite {
            fid,
            offset: 3,
            bytes: vec![1, 2, 3],
        },
        Procedure::SimDeliverBreaks,
        Procedure::SimReset {
            client: ClientId(4),
        },
        Procedure::SimProbe {
            client: ClientId(5),
        },
        Procedure::SimForge {
            from: Addr([192, 0, 2, 9]),
            to: ClientId(0),
            procedure: CbProc::CallBack { fids: vec![fid] },
        },
        Procedure::SimForge {
            from: Addr([10, 0, 0, 2]),
            to: ClientId(0),
            procedure: CbProc::InitState(Reset::Three),
        },
    ];
    procedures
        .into_iter()
        .enumerate()
        .map(|(index, procedure)| Request {
            to: Addr([10, 0, 0, u8::try_from(index).unwrap()]),
            service: if index < 3 { Service::Vl } else { Service::Fs },
            client: ClientId(1),
            context: ContextId(u32::MAX),
            procedure,
        })
        .collect()
}

fn status(kind: FileKind) -> Status {
    Status {
        kind,
        link_count: 1,
        length: (1 << 32) + 5,
        data_version: u64::MAX,
        author: -1,
        owner: i32::MIN,
        group: 7,
        caller_access: 0xdead_beef,
        anonymous_access: 0,
        mode: 0o100_644,
        parent_vnode: 1,
        parent_unique: 1,
        client_mtime: 10,
        server_mtime: 11,
    }
}

fn answers() -> Vec<Answer> {
    let sync = VolSync { incarnation: 3 };
    let promise = Promise { lifetime: 7200 };
    vec![
        Answer::Abort(-455),
        Answer::Abort(363_524),
        Answer::Result(Results::VlEntry(VlEntry {
            name: b"user.a".to_vec(),
            ids: VolumeIds {
                read_write: Some(VolumeId(1)),
                read_only: None,
                backup: Some(VolumeId(3)),
            },
            sites: vec![
                Site {
                    server: SiteServer::Addr(Addr([10, 0, 0, 1])),
                    partition: 0,
                    holds: VolumeKind::ReadWrite,
                    release: Release::Released,
                },
                Site {
                    server: SiteServer::Uuid(ServerUuid([7; 16])),
                    partition: 25,
                    holds: VolumeKind::ReadOnly,
                    release: Release::Old,
                },
            ],
        })),
        Answer::Result(Results::Addrs(vec![Addr([1, 2, 3, 4]), Addr([5, 6, 7, 8])])),
        Answer::Result(Results::Status {
            status: status(FileKind::File),
            promise,
            sync,
        }),
        Answer::Result(Results::Bulk {
            statuses: vec![
                status(FileKind::Directory),
                status(FileKind::Symlink),
                status(FileKind::Unclassified(9)),
            ],
            promises: vec![promise, promise],
            sync,
        }),
        Answer::Result(Results::Data {
            data: vec![0, 255, 7],
            status: status(FileKind::File),
            promise,
            sync,
        }),
        Answer::Result(Results::VolumeStatus(VolumeStatus {
            volume: VolumeId(5),
            name: b"v".to_vec(),
            quota: 1,
            blocks_in_use: 2,
            partition_free: 3,
            partition_total: 4,
            offline_message: Vec::new(),
            motd: b"hi".to_vec(),
        })),
        Answer::Result(Results::Wrote { data_version: 2 }),
        Answer::Result(Results::Started { pushes: 3 }),
    ]
}

#[test]
fn every_request_round_trips() {
    for request in requests() {
        let bytes = encode_request(&request);
        assert_eq!(decode_request(&bytes), Ok(request));
    }
}

#[test]
fn every_answer_round_trips() {
    for answer in answers() {
        let bytes = encode_answer(&answer);
        assert_eq!(decode_answer(&bytes), Ok(answer));
    }
}

#[test]
fn decode_rejects_truncated_and_trailing() {
    for request in requests() {
        let bytes = encode_request(&request);
        assert_ne!(bytes, Vec::<u8>::new());
        for end in 0..bytes.len() {
            assert_eq!(
                decode_request(&bytes[..end]),
                Err(EnvelopeError::Truncated),
                "{request:?} cut at {end}"
            );
        }
        let mut long = bytes.clone();
        long.push(0);
        assert_eq!(decode_request(&long), Err(EnvelopeError::Trailing));
    }
    for answer in answers() {
        let bytes = encode_answer(&answer);
        assert_ne!(bytes, Vec::<u8>::new());
        for end in 0..bytes.len() {
            assert_eq!(
                decode_answer(&bytes[..end]),
                Err(EnvelopeError::Truncated),
                "{answer:?} cut at {end}"
            );
        }
        let mut long = bytes.clone();
        long.push(0);
        assert_eq!(decode_answer(&long), Err(EnvelopeError::Trailing));
    }
}

#[test]
fn decode_rejects_unknown_tags_and_counts_past_the_end() {
    assert_eq!(decode_answer(&[0x7f]), Err(EnvelopeError::Tag));
    assert_eq!(decode_request(&[0, 0, 0, 0, 9]), Err(EnvelopeError::Tag));
    // An abort tag, then a count that claims more bytes than follow.
    let mut bytes = encode_answer(&Answer::Result(Results::Addrs(Vec::new())));
    bytes.truncate(bytes.len() - 4);
    bytes.extend_from_slice(&u32::MAX.to_be_bytes());
    assert_eq!(decode_answer(&bytes), Err(EnvelopeError::Truncated));
}
