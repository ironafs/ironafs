// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The VL and fileserver models driven through the simulator, one
//! scenario per behaviour they serve (`docs/specs/vl-fs-read.md`), with
//! requests scripted in each test. Every
//! scenario runs twice from one seed and must give byte-identical traces.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use ironafs_sim::model::cell::{Cell, serve_in};
use ironafs_sim::model::codes::{
    SERVER_RESTARTING, UNKNOWN_PROCEDURE, VL_NO_ENTRY, VOLUME_BUSY, VOLUME_MOVED,
};
use ironafs_sim::model::envelope::{decode_answer, encode_request};
use ironafs_sim::model::fs::{BulkFault, FileServer, FileSpec};
use ironafs_sim::model::types::{
    Addr, Answer, Busy, ClientId, ContextId, Fid, FileKind, ProcName, Procedure, Release, Request,
    Results, Service, Site, SiteServer, Status, VlEntry, VolumeId, VolumeIds, VolumeKind,
};
use ironafs_sim::sched::{Config, SimDeadline, TaskCx, Wait, Waited, run};
use ironafs_sim::trace::terminal;
use ironafs_time::BootDeadline;

const VL: Addr = Addr([10, 0, 0, 1]);
const FS1: Addr = Addr([10, 0, 0, 2]);
const FS2: Addr = Addr([10, 0, 0, 3]);
const GIB2: u64 = 1 << 31;
const GIB4: u64 = 1 << 32;
const ME: ClientId = ClientId(0);
const PEER: ClientId = ClientId(1);

type Shared = Arc<Mutex<Cell>>;
type Log = Arc<Mutex<Vec<Result<Answer, Waited>>>>;
type Script = fn(&TaskCx, &Shared, &Log);

fn small() -> Fid {
    Fid::new(100, 2, 1)
}

fn big() -> Fid {
    Fid::new(100, 4, 1)
}

/// One VL server; volume `user.a` (100) on FS1 with a 10-byte file and
/// one of 4 GiB + 100 bytes; FS2 with no volume.
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
    assert!(one.add_file(small(), FileSpec::file(10)));
    assert!(one.add_file(big(), FileSpec::file(GIB4 + 100)));
    cell.add_file_server(one);
    cell.add_file_server(FileServer::new(FS2));
    cell
}

/// Sends `procedure` to `to`'s `service` as `client`, through the
/// simulator, and logs what came back.
fn ask(
    cx: &TaskCx,
    log: &Log,
    to: Addr,
    service: Service,
    client: ClientId,
    procedure: Procedure,
) -> Result<Answer, Waited> {
    cx.op("ask", procedure.name().name(), None);
    let request = encode_request(&Request {
        to,
        service,
        client,
        context: ContextId(1),
        procedure,
    });
    let now = cx.clock().boot().unwrap();
    let deadline = SimDeadline::Boot(BootDeadline::after(now, Duration::from_secs(1)).unwrap());
    let answer = cx
        .call(&request, deadline)
        .map(|reply| decode_answer(&reply).unwrap());
    log.lock().unwrap().push(answer.clone());
    answer
}

fn fs(cx: &TaskCx, log: &Log, to: Addr, procedure: Procedure) -> Result<Answer, Waited> {
    ask(cx, log, to, Service::Fs, ME, procedure)
}

fn vl(cx: &TaskCx, log: &Log, procedure: Procedure) -> Result<Answer, Waited> {
    ask(cx, log, VL, Service::Vl, ME, procedure)
}

fn lookup() -> Procedure {
    Procedure::VlGetEntryByName {
        name: b"user.a".to_vec(),
    }
}

fn fetch64(fid: Fid, offset: u64, length: u64) -> Procedure {
    Procedure::FsFetchData64 {
        fid,
        offset,
        length,
    }
}

/// Yields once.
fn pause(cx: &TaskCx) {
    let now = cx.clock().boot().unwrap();
    let waited = cx.wait(Wait {
        signal: None,
        cancel: None,
        deadline: SimDeadline::Boot(BootDeadline::after(now, Duration::ZERO).unwrap()),
    });
    assert_eq!(waited, Waited::DeadlinePassed);
}

/// Runs `tasks` against a fresh `cell()` at `seed`; returns the trace,
/// each task's log, and the cell as the run left it.
fn simulate(name: &'static str, seed: u64, tasks: &[Script]) -> (String, Vec<Log>, Cell) {
    let shared: Shared = Arc::new(Mutex::new(cell()));
    let logs: Vec<Log> = tasks.iter().map(|_| Log::default()).collect();
    let outcome = run(&Config::new(name, seed), None, None, |setup| {
        serve_in(setup, &shared);
        for (script, log) in tasks.iter().zip(&logs) {
            let (script, log, cell) = (*script, Arc::clone(log), Arc::clone(&shared));
            setup.task("client", move |cx: &TaskCx| script(cx, &cell, &log));
        }
    });
    assert_eq!(outcome.failure, None, "{}", outcome.trace);
    assert_eq!(terminal(&outcome.trace), Some("end ok"));
    let cell = shared.lock().unwrap().clone();
    (outcome.trace, logs, cell)
}

/// Runs the scenario twice per seed and checks the traces are
/// byte-identical; returns the first run's logs and cell for seed 1.
fn deterministic(name: &'static str, tasks: &[Script]) -> (Vec<Vec<Result<Answer, Waited>>>, Cell) {
    let mut first = None;
    for seed in 1..=3 {
        let (trace, logs, cell) = simulate(name, seed, tasks);
        let (again, _, _) = simulate(name, seed, tasks);
        assert_eq!(trace, again, "seed {seed}");
        assert!(trace.contains(" world reply c0 "), "seed {seed}");
        if first.is_none() {
            let logs = logs.iter().map(|log| log.lock().unwrap().clone()).collect();
            first = Some((logs, cell));
        }
    }
    first.unwrap()
}

fn status(answer: &Result<Answer, Waited>) -> Status {
    match answer {
        Ok(Answer::Result(Results::Status { status, .. } | Results::Data { status, .. })) => {
            *status
        }
        other => panic!("no status: {other:?}"),
    }
}

fn data(answer: &Result<Answer, Waited>) -> Vec<u8> {
    match answer {
        Ok(Answer::Result(Results::Data { data, .. })) => data.clone(),
        other => panic!("no data: {other:?}"),
    }
}

fn sites(answer: &Result<Answer, Waited>) -> Vec<SiteServer> {
    match answer {
        Ok(Answer::Result(Results::VlEntry(entry))) => {
            entry.sites.iter().map(|site| site.server).collect()
        }
        other => panic!("no entry: {other:?}"),
    }
}

#[test]
fn vl_lookups_follow_a_move_and_an_offline_server_drops_its_calls() {
    let (logs, _) = deterministic(
        "models-vl",
        &[|cx, cell, log| {
            vl(cx, log, lookup()).unwrap();
            let _ = vl(
                cx,
                log,
                Procedure::VlGetEntryByName {
                    name: b"user.b".to_vec(),
                },
            );
            let _ = vl(
                cx,
                log,
                Procedure::VlGetEntryById {
                    volume: VolumeId(100),
                    kind: VolumeKind::ReadWrite,
                },
            );
            fs(cx, log, FS1, Procedure::FsFetchStatus { fid: small() }).unwrap();
            cx.op("move", "user.a", None);
            assert!(
                cell.lock()
                    .unwrap()
                    .move_volume(b"user.a", VolumeId(100), FS2)
            );
            let _ = fs(cx, log, FS1, Procedure::FsFetchStatus { fid: small() });
            let _ = vl(cx, log, lookup());
            let _ = fs(cx, log, FS2, Procedure::FsFetchStatus { fid: small() });
            cx.op("down", "fs2", None);
            cell.lock().unwrap().set_down(FS2, true);
            let _ = fs(cx, log, FS2, Procedure::FsFetchStatus { fid: small() });
            cell.lock().unwrap().set_down(VL, true);
            let _ = vl(cx, log, lookup());
            cell.lock().unwrap().set_down(VL, false);
            cell.lock()
                .unwrap()
                .vldb()
                .refuse(ProcName::VlGetEntryByName);
            let _ = vl(cx, log, lookup());
        }],
    );
    let log = &logs[0];
    assert_eq!(sites(&log[0]), vec![SiteServer::Addr(FS1)]);
    assert_eq!(log[1], Ok(Answer::Abort(VL_NO_ENTRY)));
    assert_eq!(sites(&log[2]), vec![SiteServer::Addr(FS1)]);
    assert_eq!(status(&log[3]).length, 10);
    assert_eq!(log[4], Ok(Answer::Abort(VOLUME_MOVED)));
    assert_eq!(sites(&log[5]), vec![SiteServer::Addr(FS2)]);
    assert_eq!(status(&log[6]).length, 10);
    assert_eq!(log[7], Err(Waited::DeadlinePassed));
    assert_eq!(log[8], Err(Waited::DeadlinePassed));
    assert_eq!(log[9], Ok(Answer::Abort(UNKNOWN_PROCEDURE)));
    assert_eq!(log.len(), 10);
}

#[test]
fn fs_serves_status_data_and_a_refused_inline_bulk_leaves_bulk_status() {
    let (logs, _) = deterministic(
        "models-fs",
        &[|cx, cell, log| {
            fs(cx, log, FS1, Procedure::FsFetchStatus { fid: small() }).unwrap();
            fs(cx, log, FS1, fetch64(small(), 0, 64)).unwrap();
            let fids = vec![small(), big()];
            fs(
                cx,
                log,
                FS1,
                Procedure::FsInlineBulkStatus { fids: fids.clone() },
            )
            .unwrap();
            cell.lock()
                .unwrap()
                .file_server(FS1)
                .unwrap()
                .refuse(ProcName::FsInlineBulkStatus);
            fs(
                cx,
                log,
                FS1,
                Procedure::FsInlineBulkStatus { fids: fids.clone() },
            )
            .unwrap();
            fs(cx, log, FS1, Procedure::FsBulkStatus { fids: fids.clone() }).unwrap();
            let faults = [BulkFault::DropLast, BulkFault::UnclassifiedAt(1)];
            for fault in faults {
                cell.lock()
                    .unwrap()
                    .file_server(FS1)
                    .unwrap()
                    .bulk_fault(Some(fault));
                fs(cx, log, FS1, Procedure::FsBulkStatus { fids: fids.clone() }).unwrap();
            }
            fs(
                cx,
                log,
                FS1,
                Procedure::FsGetVolumeStatus {
                    volume: VolumeId(100),
                },
            )
            .unwrap();
        }],
    );
    let log = &logs[0];
    assert_eq!(status(&log[0]).kind, FileKind::File);
    assert_eq!(data(&log[1]).len(), 10);
    let kinds = |answer: &Result<Answer, Waited>| match answer {
        Ok(Answer::Result(Results::Bulk {
            statuses, promises, ..
        })) => {
            assert_eq!(statuses.len(), promises.len());
            statuses
                .iter()
                .map(|status| status.kind)
                .collect::<Vec<_>>()
        }
        other => panic!("no bulk: {other:?}"),
    };
    assert_eq!(kinds(&log[2]), vec![FileKind::File, FileKind::File]);
    assert_eq!(log[3], Ok(Answer::Abort(UNKNOWN_PROCEDURE)));
    assert_eq!(kinds(&log[4]), vec![FileKind::File, FileKind::File]);
    assert_eq!(kinds(&log[5]), vec![FileKind::File]);
    assert_eq!(kinds(&log[6])[1], FileKind::Unclassified(0xff));
    assert!(matches!(
        log[7],
        Ok(Answer::Result(Results::VolumeStatus(_)))
    ));
}

#[test]
fn fetch64_at_and_across_2_and_4_gib_and_a_server_that_refuses_it() {
    let (logs, cell) = deterministic(
        "models-fetch",
        &[|cx, cell, log| {
            for offset in [GIB2 - 8, GIB2, GIB4 - 8, GIB4, GIB4 + 92] {
                fs(cx, log, FS1, fetch64(big(), offset, 16)).unwrap();
            }
            cell.lock()
                .unwrap()
                .file_server(FS1)
                .unwrap()
                .refuse(ProcName::FsFetchData64);
            fs(cx, log, FS1, fetch64(big(), 0, 16)).unwrap();
            fs(
                cx,
                log,
                FS1,
                Procedure::FsFetchData {
                    fid: big(),
                    offset: u32::try_from(GIB2 - 17).unwrap(),
                    length: 16,
                },
            )
            .unwrap();
        }],
    );
    let log = &logs[0];
    let mut cell = cell;
    let server = cell.file_server(FS1).unwrap();
    for (index, offset) in [GIB2 - 8, GIB2, GIB4 - 8, GIB4].into_iter().enumerate() {
        assert_eq!(Some(data(&log[index])), server.content(big(), offset, 16));
    }
    // Eight bytes are left at 4 GiB + 92 of a file of 4 GiB + 100.
    assert_eq!(data(&log[4]), server.content(big(), GIB4 + 92, 8).unwrap());
    assert_eq!(status(&log[4]).length, GIB4 + 100);
    assert_eq!(log[5], Ok(Answer::Abort(UNKNOWN_PROCEDURE)));
    assert_eq!(Some(data(&log[6])), server.content(big(), GIB2 - 17, 16));
}

#[test]
fn a_busy_server_answers_busy_then_serves() {
    let (logs, _) = deterministic(
        "models-busy",
        &[|cx, cell, log| {
            {
                let mut cell = cell.lock().unwrap();
                cell.file_server(FS1).unwrap().busy(Busy {
                    code: VOLUME_BUSY,
                    calls: 1,
                });
                cell.vldb().busy(Busy {
                    code: SERVER_RESTARTING,
                    calls: 1,
                });
            }
            for _ in 0..2 {
                fs(cx, log, FS1, Procedure::FsFetchStatus { fid: small() }).unwrap();
                vl(cx, log, lookup()).unwrap();
            }
        }],
    );
    let log = &logs[0];
    assert_eq!(log[0], Ok(Answer::Abort(VOLUME_BUSY)));
    assert_eq!(log[1], Ok(Answer::Abort(SERVER_RESTARTING)));
    assert_eq!(status(&log[2]).length, 10);
    assert_eq!(sites(&log[3]), vec![SiteServer::Addr(FS1)]);
}

/// Yields until `ready` holds of the cell: the reader and the writer
/// hand over through the server's own state, so one seed gives one
/// outcome whatever the schedule between the handovers.
fn await_cell(cx: &TaskCx, cell: &Shared, ready: fn(&mut FileServer) -> bool) {
    for _ in 0..10_000 {
        if ready(cell.lock().unwrap().file_server(FS1).unwrap()) {
            return;
        }
        pause(cx);
    }
    panic!("the cell never became ready");
}

#[test]
fn a_second_clients_write_bumps_the_data_version_and_owes_a_break() {
    let reader: Script = |cx, cell, log| {
        fs(cx, log, FS1, fetch64(small(), 0, 10)).unwrap();
        await_cell(cx, cell, |server| !server.owed_breaks().is_empty());
        fs(cx, log, FS1, fetch64(small(), 0, 12)).unwrap();
    };
    let writer: Script = |cx, cell, log| {
        await_cell(cx, cell, |server| server.promise_holders(small()) == [ME]);
        ask(
            cx,
            log,
            FS1,
            Service::Fs,
            PEER,
            Procedure::PeerWrite {
                fid: small(),
                offset: 8,
                bytes: vec![0xaa; 4],
            },
        )
        .unwrap();
    };
    let (logs, cell) = deterministic("models-write", &[reader, writer]);
    let (read, wrote) = (&logs[0], &logs[1]);
    assert_eq!(status(&read[0]).data_version, 1);
    assert_eq!(
        wrote[0],
        Ok(Answer::Result(Results::Wrote { data_version: 2 }))
    );
    let after = status(&read[1]);
    assert_eq!((after.data_version, after.length), (2, 12));
    let bytes = data(&read[1]);
    assert_eq!(bytes[..8], data(&read[0])[..8]);
    assert_eq!(bytes[8..], [0xaa; 4]);
    let mut cell = cell;
    let server = cell.file_server(FS1).unwrap();
    // The write ended the reader's promise and owes it a break; the
    // reader's fetch after the write took a new one.
    assert_eq!(server.owed_breaks(), &[(ME, small())]);
    assert_eq!(server.promise_holders(small()), vec![ME]);
}

/// The workspace crates holding a client codec: a model that shared one
/// with the client would hide a codec bug in both.
const CLIENT_CODECS: &[&str] = &[
    "ironafs-xdr",
    "ironafs-rx",
    "ironafs-rxkad",
    "ironafs-rxgk",
    "ironafs-proto",
    "ironafs-client",
    "ironafs-cache",
];

/// The client codec crates `manifest` names as a dependency of any kind.
fn codec_dependencies(manifest: &str) -> Vec<&'static str> {
    let mut found = Vec::new();
    let mut in_dependencies = false;
    for line in manifest.lines().map(str::trim) {
        if line.starts_with('[') {
            in_dependencies = line.trim_matches(['[', ']']).ends_with("dependencies");
            continue;
        }
        if !in_dependencies {
            continue;
        }
        let name = line.split(['=', '.', ' ']).next().unwrap_or_default();
        if let Some(codec) = CLIENT_CODECS.iter().find(|codec| **codec == name) {
            found.push(*codec);
        }
    }
    found
}

#[test]
fn the_models_depend_on_no_client_codec_crate() {
    let manifest = include_str!("../../Cargo.toml");
    assert_eq!(codec_dependencies(manifest), Vec::<&str>::new());
    // The check sees a planted dependency in each form.
    let planted = format!(
        "{manifest}\n[dev-dependencies]\nironafs-xdr.workspace = true\n\
         [target.'cfg(unix)'.dependencies]\nironafs-proto = {{ path = \"x\" }}\n"
    );
    assert_eq!(
        codec_dependencies(&planted),
        vec!["ironafs-xdr", "ironafs-proto"]
    );
}
