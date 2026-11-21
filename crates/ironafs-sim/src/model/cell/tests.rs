// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The cell routes each request by address and service, drops what a
//! down or absent server would never answer, and moves a volume.

use std::sync::{Arc, Mutex};

use super::{Cell, CellWorld, NETWORK, PUSH_ATTEMPTS, PUSH_INTERVAL, PUSH_TIMEOUT};
use crate::model::callback::{
    CbAnswer, CbKind, CbProc, CbRequest, Reset, decode_cb_request, encode_cb_answer,
};
use crate::model::codes::{MARSHALLING, UNKNOWN_PROCEDURE, VOLUME_MOVED};
use crate::model::envelope::{decode_answer, encode_request};
use crate::model::fs::{FileServer, FileSpec};
use crate::model::types::{
    Addr, Answer, ClientId, ContextId, Fid, Procedure, Release, Request, Results, Service, Site,
    SiteServer, VlEntry, VolumeId, VolumeIds, VolumeKind,
};
use crate::sched::{Served, World};

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

const ME: ClientId = ClientId(0);
const PEER: ClientId = ClientId(1);
const ABSENT: Addr = Addr([192, 0, 2, 9]);

fn shared() -> Arc<Mutex<Cell>> {
    Arc::new(Mutex::new(cell()))
}

fn bytes_from(client: ClientId, to: Addr, procedure: Procedure) -> Vec<u8> {
    encode_request(&Request {
        to,
        service: Service::Fs,
        client,
        context: ContextId(1),
        procedure,
    })
}

fn reply(served: Served) -> Answer {
    match served {
        Served::Reply(bytes) => decode_answer(&bytes).unwrap(),
        other => panic!("not a reply: {other:?}"),
    }
}

fn started(count: u32) -> Answer {
    Answer::Result(Results::Started { pushes: count })
}

#[test]
fn the_world_holds_a_lost_clients_call_until_its_reset_is_answered() {
    let cell = shared();
    let mut world = CellWorld::new(&cell);
    reply(world.serve(0, &bytes_from(ME, FS1, status())));
    let write = Procedure::PeerWrite {
        fid: fid(),
        offset: 0,
        bytes: b"x".to_vec(),
    };
    reply(world.serve(1, &bytes_from(PEER, FS1, write)));
    assert_eq!(
        reply(world.serve(2, &bytes_from(PEER, FS1, Procedure::SimDeliverBreaks))),
        started(1)
    );
    let polled = world.poll();
    let push = polled.pushes.first().unwrap();
    assert_eq!(polled.pushes.len(), 1);
    assert_eq!((push.endpoint, push.attempts), (0, PUSH_ATTEMPTS));
    assert_eq!((push.interval, push.timeout), (PUSH_INTERVAL, PUSH_TIMEOUT));
    assert_eq!(
        decode_cb_request(&push.request).unwrap(),
        CbRequest {
            from: FS1,
            to: ME,
            procedure: CbProc::CallBack { fids: vec![fid()] },
        }
    );
    assert!(world.admit(push.token, 1));
    world.settled(push.token, None);

    assert_eq!(world.serve(3, &bytes_from(ME, FS1, status())), Served::Hold);
    let polled = world.poll();
    let reset = polled.pushes.first().unwrap();
    assert_eq!(
        decode_cb_request(&reset.request).unwrap().procedure,
        CbProc::InitState(Reset::One)
    );
    assert_eq!(polled.releases, []);
    world.settled(reset.token, Some(encode_cb_answer(&CbAnswer::Empty)));
    let polled = world.poll();
    let (released, bytes) = polled.releases.first().unwrap();
    assert_eq!(*released, 3);
    assert!(matches!(
        decode_answer(bytes.as_ref().unwrap()).unwrap(),
        Answer::Result(Results::Status { .. })
    ));
}

#[test]
fn a_forged_call_is_pushed_by_the_network_and_no_server_hears_its_answer() {
    let cell = shared();
    let mut world = CellWorld::new(&cell);
    let forge = Procedure::SimForge {
        from: FS1,
        to: ME,
        procedure: CbProc::CallBack { fids: vec![fid()] },
    };
    assert_eq!(
        reply(world.serve(0, &bytes_from(PEER, NETWORK, forge))),
        started(1)
    );
    let polled = world.poll();
    let push = polled.pushes.first().unwrap();
    let request = decode_cb_request(&push.request).unwrap();
    assert_eq!(request.from, FS1);
    world.settled(push.token, Some(encode_cb_answer(&CbAnswer::Empty)));
    let cell = cell.lock().unwrap();
    assert_eq!(cell.forged_answers(), &[(request, Some(CbAnswer::Empty))]);
    let mut cell = cell.clone();
    assert_eq!(cell.file_server(FS1).unwrap().delivered(), []);
}

#[test]
fn callback_faults_lose_the_attempts_they_name_for_their_kind_and_client() {
    let cell = shared();
    cell.lock().unwrap().drop_callbacks(ME, CbKind::Break, 2);
    let mut world = CellWorld::new(&cell);
    for (client, procedure) in [
        (ME, CbProc::CallBack { fids: vec![fid()] }),
        (ME, CbProc::Probe),
        (PEER, CbProc::CallBack { fids: vec![fid()] }),
    ] {
        let forge = Procedure::SimForge {
            from: FS1,
            to: client,
            procedure,
        };
        reply(world.serve(0, &bytes_from(PEER, NETWORK, forge)));
    }
    let tokens: Vec<u64> = world.poll().pushes.iter().map(|push| push.token).collect();
    let admitted = |world: &mut CellWorld, token: u64| -> Vec<bool> {
        (1..=3).map(|attempt| world.admit(token, attempt)).collect()
    };
    assert_eq!(admitted(&mut world, tokens[0]), [false, false, true]);
    assert_eq!(admitted(&mut world, tokens[1]), [true, true, true]);
    assert_eq!(admitted(&mut world, tokens[2]), [true, true, true]);
}

#[test]
fn the_world_drops_calls_to_a_down_server_and_answers_garbage_with_a_marshalling_abort() {
    let cell = shared();
    cell.lock().unwrap().set_down(FS1, true);
    let mut world = CellWorld::new(&cell);
    assert_eq!(world.serve(0, &bytes_from(ME, FS1, status())), Served::Drop);
    let mut junk = FS2.0.to_vec();
    junk.extend_from_slice(b"junk");
    assert_eq!(reply(world.serve(1, &junk)), Answer::Abort(MARSHALLING));
}

#[test]
fn the_world_drops_bytes_to_an_absent_or_down_server_unread() {
    let cell = shared();
    cell.lock().unwrap().set_down(FS1, true);
    let mut world = CellWorld::new(&cell);
    let mut junk = ABSENT.0.to_vec();
    junk.extend_from_slice(b"junk");
    assert_eq!(world.serve(0, &junk), Served::Drop);
    let mut junk = FS1.0.to_vec();
    junk.extend_from_slice(b"junk");
    assert_eq!(world.serve(1, &junk), Served::Drop);
}

#[test]
fn only_the_network_address_forges_and_it_serves_nothing_else() {
    let cell = shared();
    let mut world = CellWorld::new(&cell);
    let forge = || Procedure::SimForge {
        from: FS1,
        to: ME,
        procedure: CbProc::Probe,
    };
    assert_eq!(
        world.serve(0, &bytes_from(PEER, ABSENT, forge())),
        Served::Drop
    );
    assert_eq!(
        reply(world.serve(1, &bytes_from(PEER, FS1, forge()))),
        Answer::Abort(UNKNOWN_PROCEDURE)
    );
    assert_eq!(
        reply(world.serve(2, &bytes_from(PEER, NETWORK, status()))),
        Answer::Abort(UNKNOWN_PROCEDURE)
    );
    assert_eq!(world.poll().pushes, []);
    assert_eq!(
        reply(world.serve(3, &bytes_from(PEER, NETWORK, forge()))),
        started(1)
    );
    assert_eq!(world.poll().pushes.len(), 1);
}

/// The bytes and the world paths share one serving policy: for every
/// routing case where the world holds nothing, they give the same answer
/// or the same drop.
#[test]
fn the_world_and_the_bytes_path_serve_alike() {
    let mut cases: Vec<Vec<u8>> = vec![
        Vec::new(),
        vec![10, 0],
        bytes_from(ME, FS1, status()),
        bytes_from(ME, FS2, status()),
        bytes_from(ME, ABSENT, status()),
        bytes_from(ME, NETWORK, status()),
        bytes_from(
            ME,
            FS1,
            Procedure::SimForge {
                from: FS1,
                to: ME,
                procedure: CbProc::Probe,
            },
        ),
        encode_request(&request(VL, Service::Vl, lookup())),
        encode_request(&request(FS1, Service::Vl, lookup())),
        encode_request(&request(VL, Service::Fs, status())),
    ];
    for to in [FS1, FS2, VL, NETWORK, ABSENT] {
        let mut junk = to.0.to_vec();
        junk.extend_from_slice(b"junk");
        cases.push(junk);
    }
    for down in [false, true] {
        let mut bytes_cell = cell();
        bytes_cell.set_down(FS1, down);
        let shared = shared();
        shared.lock().unwrap().set_down(FS1, down);
        let mut world = CellWorld::new(&shared);
        for (index, case) in cases.iter().enumerate() {
            let via_bytes = bytes_cell.serve_bytes(case);
            let via_world = match world.serve(u64::try_from(index).unwrap(), case) {
                Served::Reply(bytes) => Some(bytes),
                Served::Drop => None,
                Served::Hold => panic!("case {index} held"),
            };
            assert_eq!(via_bytes, via_world, "case {index}, down {down}");
        }
    }
}
