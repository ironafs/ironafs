// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The cell routes each request by address and service, drops what a
//! down or absent server would never answer, and moves a volume.

use super::Cell;
use crate::model::codes::{MARSHALLING, UNKNOWN_PROCEDURE, VOLUME_MOVED};
use crate::model::envelope::{decode_answer, encode_request};
use crate::model::fs::{FileServer, FileSpec};
use crate::model::types::{
    Addr, Answer, ClientId, ContextId, Fid, Procedure, Release, Request, Results, Service, Site,
    SiteServer, VlEntry, VolumeId, VolumeIds, VolumeKind,
};

const VL: Addr = Addr([10, 0, 0, 1]);
const FS1: Addr = Addr([10, 0, 0, 2]);
const FS2: Addr = Addr([10, 0, 0, 3]);

fn fid() -> Fid {
    Fid::new(100, 2, 1)
}

fn cell() -> Cell {
    let mut cell = Cell::new();
    cell.add_vl_server(VL);
    cell.vldb().add_volume(VlEntry {
        name: b"user.a".to_vec(),
        ids: VolumeIds {
            read_write: Some(VolumeId(100)),
            ..VolumeIds::default()
        },
        sites: vec![Site {
            server: SiteServer::Addr(FS1),
            partition: 0,
            holds: VolumeKind::ReadWrite,
            release: Release::Released,
        }],
    });
    let mut one = FileServer::new(FS1);
    one.add_volume(VolumeId(100), b"user.a");
    one.add_file(fid(), FileSpec::file(10));
    cell.add_file_server(one);
    cell.add_file_server(FileServer::new(FS2));
    cell
}

fn request(to: Addr, service: Service, procedure: Procedure) -> Request {
    Request {
        to,
        service,
        client: ClientId(0),
        context: ContextId(1),
        procedure,
    }
}

fn lookup() -> Procedure {
    Procedure::VlGetEntryByName {
        name: b"user.a".to_vec(),
    }
}

fn status() -> Procedure {
    Procedure::FsFetchStatus { fid: fid() }
}

#[test]
fn requests_reach_the_server_and_service_they_name() {
    let mut cell = cell();
    assert!(matches!(
        cell.serve(&request(VL, Service::Vl, lookup())),
        Some(Answer::Result(Results::VlEntry(_)))
    ));
    assert!(matches!(
        cell.serve(&request(FS1, Service::Fs, status())),
        Some(Answer::Result(Results::Status { .. }))
    ));
    // The file service of a VL server, and the VL service of a
    // fileserver, hold no such service: dropped.
    assert_eq!(cell.serve(&request(VL, Service::Fs, status())), None);
    assert_eq!(cell.serve(&request(FS1, Service::Vl, lookup())), None);
    // A VL procedure sent to the file service is unknown there.
    assert_eq!(
        cell.serve(&request(FS1, Service::Fs, lookup())),
        Some(Answer::Abort(UNKNOWN_PROCEDURE))
    );
    assert_eq!(
        cell.serve(&request(Addr([10, 9, 9, 9]), Service::Fs, status())),
        None
    );
}

#[test]
fn a_down_server_drops_every_request_until_it_is_back() {
    let mut cell = cell();
    cell.set_down(FS1, true);
    assert_eq!(cell.serve(&request(FS1, Service::Fs, status())), None);
    cell.set_down(VL, true);
    assert_eq!(cell.serve(&request(VL, Service::Vl, lookup())), None);
    cell.set_down(FS1, false);
    assert!(cell.serve(&request(FS1, Service::Fs, status())).is_some());
}

#[test]
fn a_moved_volume_is_served_by_its_new_server_and_refused_as_moved_by_the_old() {
    let mut cell = cell();
    assert!(cell.move_volume(b"user.a", VolumeId(100), FS2));
    assert_eq!(
        cell.serve(&request(FS1, Service::Fs, status())),
        Some(Answer::Abort(VOLUME_MOVED))
    );
    let Some(Answer::Result(Results::Status { status, .. })) =
        cell.serve(&request(FS2, Service::Fs, status()))
    else {
        panic!("the new server does not serve the volume");
    };
    assert_eq!(status.length, 10);
    let Some(Answer::Result(Results::VlEntry(entry))) =
        cell.serve(&request(VL, Service::Vl, lookup()))
    else {
        panic!("no entry");
    };
    assert_eq!(entry.sites.len(), 1);
    assert_eq!(entry.sites[0].server, SiteServer::Addr(FS2));
    assert!(!cell.move_volume(b"user.a", VolumeId(100), Addr([1, 1, 1, 1])));
    assert!(!cell.move_volume(b"user.b", VolumeId(100), FS1));
}

#[test]
fn envelope_bytes_are_answered_or_dropped() {
    let mut cell = cell();
    let bytes = encode_request(&request(FS1, Service::Fs, status()));
    let answer = cell.serve_bytes(&bytes).map(|reply| decode_answer(&reply));
    assert!(matches!(
        answer,
        Some(Ok(Answer::Result(Results::Status { .. })))
    ));
    cell.set_down(FS1, true);
    assert_eq!(cell.serve_bytes(&bytes), None);
}

#[test]
fn malformed_bytes_get_a_marshalling_abort_only_from_a_server_that_is_up() {
    let mut cell = cell();
    // An address prefix, then bytes that are no request.
    let garbage = |to: Addr| [&to.0[..], &[0xee, 0xee]].concat();
    assert_eq!(
        cell.serve_bytes(&garbage(FS1))
            .map(|reply| decode_answer(&reply)),
        Some(Ok(Answer::Abort(MARSHALLING)))
    );
    assert_eq!(
        cell.serve_bytes(&garbage(VL))
            .map(|reply| decode_answer(&reply)),
        Some(Ok(Answer::Abort(MARSHALLING)))
    );
    // Bytes to a down server, or to an address no server holds, never
    // reach a server: dropped, whatever they contain.
    cell.set_down(FS1, true);
    assert_eq!(cell.serve_bytes(&garbage(FS1)), None);
    assert_eq!(cell.serve_bytes(&garbage(Addr([10, 9, 9, 9]))), None);
    // Too short to name any server: dropped.
    assert_eq!(cell.serve_bytes(&[1, 2]), None);
}
