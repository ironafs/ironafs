// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The fileserver model serves status, bulk status and data at every
//! offset a 64-bit file has, refuses, delays and fails on demand, and
//! records promises and the breaks a peer's write owes.

use super::{BulkFault, FETCH_MAX, FileServer, FileSpec, PROMISE_LIFETIME, VolumeState};
use crate::model::callback::{CbAnswer, CbProc, CbRequest, Reset};
use crate::model::codes::{
    NO_VNODE, NOT_ON_SERVER, UNKNOWN_PROCEDURE, VOLUME_BUSY, VOLUME_MOVED, VOLUME_OFFLINE,
};
use crate::model::types::{
    Addr, Answer, Busy, ClientId, ContextId, Fid, FileKind, ProcName, Procedure, Promise, Results,
    Status, VolumeId,
};

const GIB2: u64 = 1 << 31;
const GIB4: u64 = 1 << 32;
const ME: ClientId = ClientId(0);
const PEER: ClientId = ClientId(1);
const CTX: ContextId = ContextId(10);

fn small() -> Fid {
    Fid::new(100, 2, 1)
}

fn big() -> Fid {
    Fid::new(100, 4, 1)
}

fn server() -> FileServer {
    let mut server = FileServer::new(Addr([10, 0, 0, 1]));
    server.add_volume(VolumeId(100), b"user.a");
    assert!(server.add_file(small(), FileSpec::file(10)));
    assert!(server.add_file(big(), FileSpec::file(GIB4 + 100)));
    server
}

fn status_of(answer: &Answer) -> Status {
    match answer {
        Answer::Result(Results::Status { status, .. } | Results::Data { status, .. }) => *status,
        other => panic!("no status: {other:?}"),
    }
}

fn data_of(answer: Answer) -> Vec<u8> {
    match answer {
        Answer::Result(Results::Data { data, .. }) => data,
        other => panic!("no data: {other:?}"),
    }
}

fn fetch64(server: &mut FileServer, fid: Fid, offset: u64, length: u64) -> Answer {
    server.serve(
        ME,
        CTX,
        &Procedure::FsFetchData64 {
            fid,
            offset,
            length,
        },
    )
}

#[test]
fn fetch_status_returns_the_record_a_promise_and_records_the_holder() {
    let mut server = server();
    server.set_access(small(), CTX, 0x7f);
    let answer = server.serve(ME, CTX, &Procedure::FsFetchStatus { fid: small() });
    let Answer::Result(Results::Status {
        status, promise, ..
    }) = answer
    else {
        panic!("{answer:?}");
    };
    assert_eq!(status.kind, FileKind::File);
    assert_eq!(status.length, 10);
    assert_eq!(status.data_version, 1);
    assert_eq!(status.caller_access, 0x7f);
    assert_eq!(
        promise,
        Promise {
            lifetime: PROMISE_LIFETIME
        }
    );
    assert_eq!(server.promise_holders(small()), vec![ME]);
    // Another context sees the file's default mask, not this one's.
    let other = server.serve(
        ME,
        ContextId(11),
        &Procedure::FsFetchStatus { fid: small() },
    );
    assert_eq!(status_of(&other).caller_access, 0);
}

#[test]
fn both_bulk_calls_return_one_item_per_file_in_order() {
    for procedure in [
        Procedure::FsBulkStatus {
            fids: vec![big(), small()],
        },
        Procedure::FsInlineBulkStatus {
            fids: vec![big(), small()],
        },
    ] {
        let mut server = server();
        let answer = server.serve(ME, CTX, &procedure);
        let Answer::Result(Results::Bulk {
            statuses, promises, ..
        }) = answer
        else {
            panic!("{answer:?}");
        };
        let lengths: Vec<u64> = statuses.iter().map(|status| status.length).collect();
        assert_eq!(lengths, vec![GIB4 + 100, 10]);
        assert_eq!(promises.len(), 2);
        assert_eq!(server.promise_holders(big()), vec![ME]);
    }
}

#[test]
fn refusing_the_inline_bulk_call_leaves_the_bulk_call_served() {
    let mut server = server();
    server.refuse(ProcName::FsInlineBulkStatus);
    let fids = vec![small()];
    assert_eq!(
        server.serve(
            ME,
            CTX,
            &Procedure::FsInlineBulkStatus { fids: fids.clone() }
        ),
        Answer::Abort(UNKNOWN_PROCEDURE)
    );
    assert!(matches!(
        server.serve(ME, CTX, &Procedure::FsBulkStatus { fids }),
        Answer::Result(Results::Bulk { .. })
    ));
}

#[test]
fn bulk_faults_drop_an_item_or_unclassify_one() {
    let mut server = server();
    let fids = vec![big(), small()];
    server.bulk_fault(Some(BulkFault::DropLast));
    let Answer::Result(Results::Bulk {
        statuses, promises, ..
    }) = server.serve(ME, CTX, &Procedure::FsBulkStatus { fids: fids.clone() })
    else {
        panic!("no bulk");
    };
    assert_eq!((statuses.len(), promises.len()), (1, 1));
    server.bulk_fault(Some(BulkFault::UnclassifiedAt(1)));
    let Answer::Result(Results::Bulk { statuses, .. }) =
        server.serve(ME, CTX, &Procedure::FsInlineBulkStatus { fids })
    else {
        panic!("no bulk");
    };
    assert_eq!(statuses.len(), 2);
    assert_eq!(statuses[0].kind, FileKind::File);
    assert!(matches!(statuses[1].kind, FileKind::Unclassified(_)));
}

#[test]
fn fetch64_at_and_across_the_2_and_4_gib_boundaries() {
    let mut server = server();
    for offset in [GIB2 - 8, GIB2, GIB4 - 8, GIB4, GIB4 + 84] {
        let data = data_of(fetch64(&mut server, big(), offset, 16));
        assert_eq!(data.len(), 16, "offset {offset}");
        assert_eq!(
            Some(data),
            server.content(big(), offset, 16),
            "offset {offset}"
        );
    }
    // The bytes on each side of a boundary differ from a constant fill.
    let across = server.content(big(), GIB4 - 8, 16).unwrap();
    assert_ne!(across[..8], across[8..]);
}

#[test]
fn fetch64_returns_what_the_length_allows_and_nothing_past_the_end() {
    let mut server = server();
    assert_eq!(
        data_of(fetch64(&mut server, big(), GIB4 + 90, 16)).len(),
        10
    );
    assert_eq!(
        data_of(fetch64(&mut server, big(), GIB4 + 100, 16)),
        Vec::<u8>::new()
    );
    assert_eq!(
        data_of(fetch64(&mut server, big(), u64::MAX, 16)),
        Vec::<u8>::new()
    );
    assert_eq!(
        data_of(fetch64(&mut server, big(), 0, FETCH_MAX + 1)).len(),
        usize::try_from(FETCH_MAX).unwrap()
    );
}

#[test]
fn fetch32_serves_the_same_bytes_as_fetch64_below_2_gib() {
    let mut server = server();
    let wide = data_of(fetch64(&mut server, big(), GIB2 - 24, 16));
    let narrow = data_of(server.serve(
        ME,
        CTX,
        &Procedure::FsFetchData {
            fid: big(),
            offset: u32::try_from(GIB2 - 24).unwrap(),
            length: 16,
        },
    ));
    assert_eq!(narrow, wide);
}

#[test]
fn a_server_refusing_fetch64_answers_unknown_procedure_and_still_serves_fetch32() {
    let mut server = server();
    server.refuse(ProcName::FsFetchData64);
    assert_eq!(
        fetch64(&mut server, big(), 0, 16),
        Answer::Abort(UNKNOWN_PROCEDURE)
    );
    let narrow = server.serve(
        ME,
        CTX,
        &Procedure::FsFetchData {
            fid: big(),
            offset: 0,
            length: 16,
        },
    );
    assert_eq!(data_of(narrow).len(), 16);
}

#[test]
fn volume_states_and_unknown_files_answer_their_codes() {
    let mut server = server();
    assert_eq!(
        server.serve(
            ME,
            CTX,
            &Procedure::FsFetchStatus {
                fid: Fid::new(100, 9, 1)
            }
        ),
        Answer::Abort(NO_VNODE)
    );
    // A known vnode with another uniquifier is no such vnode either.
    assert_eq!(
        server.serve(
            ME,
            CTX,
            &Procedure::FsFetchStatus {
                fid: Fid::new(100, 2, 2)
            }
        ),
        Answer::Abort(NO_VNODE)
    );
    assert_eq!(
        server.serve(
            ME,
            CTX,
            &Procedure::FsFetchStatus {
                fid: Fid::new(200, 2, 1)
            }
        ),
        Answer::Abort(NOT_ON_SERVER)
    );
    for (state, code) in [
        (VolumeState::Offline, VOLUME_OFFLINE),
        (VolumeState::Moved, VOLUME_MOVED),
    ] {
        assert!(server.set_volume_state(VolumeId(100), state));
        assert_eq!(fetch64(&mut server, small(), 0, 1), Answer::Abort(code));
        assert_eq!(
            server.serve(
                ME,
                CTX,
                &Procedure::FsBulkStatus {
                    fids: vec![small()]
                }
            ),
            Answer::Abort(code)
        );
    }
    assert!(!server.set_volume_state(VolumeId(200), VolumeState::Online));
}

#[test]
fn a_busy_spell_answers_its_calls_then_ends() {
    let mut server = server();
    server.busy(Busy {
        code: VOLUME_BUSY,
        calls: 1,
    });
    let status = Procedure::FsFetchStatus { fid: small() };
    assert_eq!(server.serve(ME, CTX, &status), Answer::Abort(VOLUME_BUSY));
    assert!(matches!(server.serve(ME, CTX, &status), Answer::Result(_)));
}

#[test]
fn a_peer_write_bumps_the_data_version_changes_the_bytes_and_owes_breaks() {
    let mut server = server();
    server.serve(ME, CTX, &Procedure::FsFetchStatus { fid: small() });
    server.serve(PEER, CTX, &Procedure::FsFetchStatus { fid: small() });
    let before = server.content(small(), 0, 10).unwrap();
    let wrote = server.serve(
        PEER,
        CTX,
        &Procedure::PeerWrite {
            fid: small(),
            offset: 8,
            bytes: vec![0xaa; 4],
        },
    );
    assert_eq!(wrote, Answer::Result(Results::Wrote { data_version: 2 }));
    let after = server.content(small(), 0, 12).unwrap();
    assert_eq!(after[..8], before[..8]);
    assert_eq!(after[8..], [0xaa; 4]);
    let status = status_of(&server.serve(ME, CTX, &Procedure::FsFetchStatus { fid: small() }));
    assert_eq!((status.data_version, status.length), (2, 12));
    assert!(status.server_mtime > 0);
    // The writer keeps its promise; the other holder is owed a break.
    assert_eq!(server.owed_breaks(), &[(ME, small())]);
    assert_eq!(server.promise_holders(small()), vec![ME, PEER]);
}

#[test]
fn a_write_past_the_end_of_u64_is_refused_and_changes_nothing() {
    let mut server = server();
    let answer = server.serve(
        PEER,
        CTX,
        &Procedure::PeerWrite {
            fid: small(),
            offset: u64::MAX,
            bytes: vec![1, 2],
        },
    );
    assert!(matches!(answer, Answer::Abort(_)));
    let status = status_of(&server.serve(ME, CTX, &Procedure::FsFetchStatus { fid: small() }));
    assert_eq!((status.data_version, status.length), (1, 10));
}

#[test]
fn volume_procedures_of_the_vl_service_are_unknown_here() {
    let mut server = server();
    assert_eq!(
        server.serve(
            ME,
            CTX,
            &Procedure::VlGetEntryByName {
                name: b"x".to_vec()
            }
        ),
        Answer::Abort(UNKNOWN_PROCEDURE)
    );
}

#[test]
fn volume_status_names_the_volume_asked() {
    let mut server = server();
    let Answer::Result(Results::VolumeStatus(status)) = server.serve(
        ME,
        CTX,
        &Procedure::FsGetVolumeStatus {
            volume: VolumeId(100),
        },
    ) else {
        panic!("no volume status");
    };
    assert_eq!(status.volume, VolumeId(100));
    assert_eq!(status.name, b"user.a");
}

fn peer_write(server: &mut FileServer, fid: Fid) {
    server.serve(
        PEER,
        CTX,
        &Procedure::PeerWrite {
            fid,
            offset: 0,
            bytes: b"new".to_vec(),
        },
    );
}

fn status_call() -> Procedure {
    Procedure::FsFetchStatus { fid: small() }
}

#[test]
fn owed_breaks_are_sent_on_demand_one_call_per_client() {
    let mut server = server();
    server.serve(ME, CTX, &status_call());
    server.serve(ME, CTX, &Procedure::FsFetchStatus { fid: big() });
    peer_write(&mut server, small());
    peer_write(&mut server, big());
    assert_eq!(server.take_outgoing(), [], "nothing is sent unasked");
    assert_eq!(server.deliver_breaks(), 1);
    let sent = server.take_outgoing();
    assert_eq!(
        sent.iter().map(|out| &out.request).collect::<Vec<_>>(),
        vec![&CbRequest {
            from: server.addr(),
            to: ME,
            procedure: CbProc::CallBack {
                fids: vec![small(), big()],
            },
        }]
    );
    assert_eq!(server.owed_breaks(), []);
    assert_eq!(server.deliver_breaks(), 0);
    let id = sent.first().unwrap().id;
    server.settled(id, Some(CbAnswer::Empty));
    assert_eq!(
        server.delivered(),
        &[(
            ME,
            CbProc::CallBack {
                fids: vec![small(), big()]
            }
        )]
    );
    assert_eq!(server.lost_clients(), []);
}

#[test]
fn resets_and_probes_are_sent_on_demand() {
    let mut server = server();
    assert_eq!(server.reset(ME), 1);
    assert_eq!(server.probe(PEER), 1);
    let sent: Vec<CbRequest> = server
        .take_outgoing()
        .into_iter()
        .map(|out| out.request)
        .collect();
    assert_eq!(
        sent,
        vec![
            CbRequest {
                from: server.addr(),
                to: ME,
                procedure: CbProc::InitState(Reset::One),
            },
            CbRequest {
                from: server.addr(),
                to: PEER,
                procedure: CbProc::Probe,
            },
        ]
    );
}

/// A14 as `docs/specs/callbacks.md` 5.8 states it: a break the server
/// cannot deliver ends its promise, and the client learns of the loss
/// at its next call, by a reset delivered before that call's answer.
#[test]
fn a_break_never_delivered_makes_the_next_call_wait_for_a_reset() {
    let mut server = server();
    server.serve(ME, CTX, &status_call());
    peer_write(&mut server, small());
    server.deliver_breaks();
    let id = server.take_outgoing().first().unwrap().id;
    server.settled(id, None);
    assert_eq!(server.failed_breaks(), &[(ME, small())]);
    assert_eq!(server.lost_clients(), vec![ME]);
    assert_eq!(server.promise_holders(small()), []);

    // The next call is held, and a reset goes out first.
    assert_eq!(server.serve_call(5, ME, CTX, &status_call()), None);
    let reset = server.take_outgoing();
    assert_eq!(
        reset
            .iter()
            .map(|out| &out.request.procedure)
            .collect::<Vec<_>>(),
        vec![&CbProc::InitState(Reset::One)]
    );
    // A second call meanwhile is held too, with no second reset.
    assert_eq!(server.serve_call(6, ME, CTX, &status_call()), None);
    assert_eq!(server.take_outgoing(), []);
    assert_eq!(server.take_released(), []);
    // Another client is served at once.
    assert!(server.serve_call(7, PEER, CTX, &status_call()).is_some());

    server.settled(reset.first().unwrap().id, Some(CbAnswer::Empty));
    let released = server.take_released();
    assert_eq!(
        released.iter().map(|(call, _)| *call).collect::<Vec<_>>(),
        vec![5, 6]
    );
    for (_, answer) in &released {
        assert_eq!(status_of(answer.as_ref().unwrap()).data_version, 2);
    }
    assert_eq!(server.lost_clients(), []);
    assert!(server.serve_call(8, ME, CTX, &status_call()).is_some());
}

#[test]
fn a_reset_that_fails_too_drops_the_held_call_and_the_client_stays_lost() {
    let mut server = server();
    server.serve(ME, CTX, &status_call());
    peer_write(&mut server, small());
    server.deliver_breaks();
    let id = server.take_outgoing().first().unwrap().id;
    server.settled(id, None);
    assert_eq!(server.serve_call(1, ME, CTX, &status_call()), None);
    let reset = server.take_outgoing().first().unwrap().id;
    server.settled(reset, None);
    assert_eq!(server.take_released(), vec![(1, None)]);
    assert_eq!(server.lost_clients(), vec![ME]);
    assert_eq!(server.serve_call(2, ME, CTX, &status_call()), None);
    assert_eq!(server.take_outgoing().len(), 1, "a new reset");
}

#[test]
fn with_a14_planted_off_a_lost_break_is_forgotten() {
    let mut server = server();
    server.plant_no_a14();
    server.serve(ME, CTX, &status_call());
    peer_write(&mut server, small());
    server.deliver_breaks();
    let id = server.take_outgoing().first().unwrap().id;
    server.settled(id, None);
    assert_eq!(server.failed_breaks(), &[(ME, small())]);
    assert_eq!(server.lost_clients(), []);
    assert!(server.serve_call(1, ME, CTX, &status_call()).is_some());
    assert_eq!(server.take_outgoing(), []);
}

#[test]
fn simulator_procedures_start_deliveries_through_serve() {
    let mut server = server();
    server.serve(ME, CTX, &status_call());
    peer_write(&mut server, small());
    assert_eq!(
        server.serve(PEER, CTX, &Procedure::SimDeliverBreaks),
        Answer::Result(Results::Started { pushes: 1 })
    );
    assert_eq!(
        server.serve(PEER, CTX, &Procedure::SimReset { client: ME }),
        Answer::Result(Results::Started { pushes: 1 })
    );
    assert_eq!(
        server.serve(PEER, CTX, &Procedure::SimProbe { client: ME }),
        Answer::Result(Results::Started { pushes: 1 })
    );
    assert_eq!(server.take_outgoing().len(), 3);
}

#[test]
fn the_data_version_is_readable_without_a_call() {
    let mut server = server();
    assert_eq!(server.data_version(small()), Some(1));
    peer_write(&mut server, small());
    assert_eq!(server.data_version(small()), Some(2));
    assert_eq!(server.data_version(Fid::new(100, 9, 9)), None);
    assert_eq!(server.promise_holders(small()), []);
}
