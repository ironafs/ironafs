// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The VL model answers lookups by name and id, resolves server
//! identities, follows a move, and refuses or delays on demand.

use super::VlDb;
use crate::model::codes::{SERVER_RESTARTING, UNKNOWN_PROCEDURE, VL_NO_ENTRY};
use crate::model::types::{
    Addr, Answer, Busy, Fid, ProcName, Procedure, Release, Results, ServerUuid, Site, SiteServer,
    VlEntry, VolumeId, VolumeIds, VolumeKind,
};

fn site(last: u8, holds: VolumeKind) -> Site {
    Site {
        server: SiteServer::Addr(Addr([10, 0, 0, last])),
        partition: 0,
        holds,
        release: Release::Released,
    }
}

fn entry() -> VlEntry {
    VlEntry {
        name: b"user.a".to_vec(),
        ids: VolumeIds {
            read_write: Some(VolumeId(100)),
            read_only: Some(VolumeId(101)),
            backup: None,
        },
        sites: vec![
            site(1, VolumeKind::ReadWrite),
            site(2, VolumeKind::ReadOnly),
        ],
    }
}

fn db() -> VlDb {
    let mut db = VlDb::new();
    db.add_volume(entry());
    db.add_server(ServerUuid([5; 16]), vec![Addr([10, 0, 0, 9])]);
    db
}

#[test]
fn lookup_by_name_returns_the_entry() {
    let answer = db().serve(&Procedure::VlGetEntryByName {
        name: b"user.a".to_vec(),
    });
    assert_eq!(answer, Answer::Result(Results::VlEntry(entry())));
}

#[test]
fn lookup_by_id_matches_the_version_asked() {
    let mut db = db();
    for (volume, kind) in [(100, VolumeKind::ReadWrite), (101, VolumeKind::ReadOnly)] {
        assert_eq!(
            db.serve(&Procedure::VlGetEntryById {
                volume: VolumeId(volume),
                kind,
            }),
            Answer::Result(Results::VlEntry(entry()))
        );
    }
    // The read/write id asked as a read-only one is no entry.
    assert_eq!(
        db.serve(&Procedure::VlGetEntryById {
            volume: VolumeId(100),
            kind: VolumeKind::ReadOnly,
        }),
        Answer::Abort(VL_NO_ENTRY)
    );
}

#[test]
fn unknown_name_is_no_such_entry() {
    assert_eq!(
        db().serve(&Procedure::VlGetEntryByName {
            name: b"user.b".to_vec(),
        }),
        Answer::Abort(VL_NO_ENTRY)
    );
}

#[test]
fn get_addrs_resolves_a_known_identity_only() {
    let mut db = db();
    assert_eq!(
        db.serve(&Procedure::VlGetAddrs {
            server: ServerUuid([5; 16]),
        }),
        Answer::Result(Results::Addrs(vec![Addr([10, 0, 0, 9])]))
    );
    assert_eq!(
        db.serve(&Procedure::VlGetAddrs {
            server: ServerUuid([6; 16]),
        }),
        Answer::Abort(VL_NO_ENTRY)
    );
}

#[test]
fn a_move_is_seen_by_the_next_lookup() {
    let mut db = db();
    assert!(db.set_sites(b"user.a", vec![site(7, VolumeKind::ReadWrite)]));
    assert!(!db.set_sites(b"user.b", Vec::new()));
    let Answer::Result(Results::VlEntry(moved)) = db.serve(&Procedure::VlGetEntryByName {
        name: b"user.a".to_vec(),
    }) else {
        panic!("no entry");
    };
    assert_eq!(moved.sites, vec![site(7, VolumeKind::ReadWrite)]);
}

#[test]
fn refused_and_file_service_procedures_are_unknown() {
    let mut db = db();
    db.refuse(ProcName::VlGetEntryByName);
    assert_eq!(
        db.serve(&Procedure::VlGetEntryByName {
            name: b"user.a".to_vec(),
        }),
        Answer::Abort(UNKNOWN_PROCEDURE)
    );
    // The refusal is of that procedure only.
    assert_eq!(
        db.serve(&Procedure::VlGetEntryById {
            volume: VolumeId(100),
            kind: VolumeKind::ReadWrite,
        }),
        Answer::Result(Results::VlEntry(entry()))
    );
    assert_eq!(
        db.serve(&Procedure::FsFetchStatus {
            fid: Fid::new(100, 1, 1),
        }),
        Answer::Abort(UNKNOWN_PROCEDURE)
    );
}

#[test]
fn a_busy_spell_answers_its_calls_then_ends() {
    let mut db = db();
    db.busy(Busy {
        code: SERVER_RESTARTING,
        calls: 2,
    });
    let lookup = Procedure::VlGetEntryByName {
        name: b"user.a".to_vec(),
    };
    assert_eq!(db.serve(&lookup), Answer::Abort(SERVER_RESTARTING));
    assert_eq!(db.serve(&lookup), Answer::Abort(SERVER_RESTARTING));
    assert_eq!(db.serve(&lookup), Answer::Result(Results::VlEntry(entry())));
}
