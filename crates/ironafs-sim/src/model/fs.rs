// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The fileserver model: the file-service read procedures of
//! `docs/specs/vl-fs-read.md` (5.1.2; 5.1.4, points 4 to 7), the abort
//! codes of its 5.9 file-service rows, the refusals that drive 5.8's
//! fallbacks, and the simulator's own write by a second client, which
//! bumps a file's data version and owes a break to every other client
//! that holds a callback promise on the file.
//!
//! A file's bytes are computed from its id and offset, with the writes
//! laid over them, so files of 4 GiB and more cost no memory.

use std::collections::{BTreeMap, BTreeSet};

use super::codes::{
    MARSHALLING, NO_VNODE, NOT_ON_SERVER, UNKNOWN_PROCEDURE, VOLUME_MOVED, VOLUME_OFFLINE,
};
use super::types::{
    Addr, Answer, Busy, ClientId, ContextId, Fid, FileKind, ProcName, Procedure, Promise, Results,
    Status, VolSync, VolumeId, VolumeStatus,
};

#[cfg(test)]
mod tests;

/// The lifetime of every callback promise the model grants, in seconds.
pub const PROMISE_LIFETIME: u32 = 7_200;

/// The most bytes one fetch reply holds: the client's own chunk limit
/// (`fs.fetch.chunk_max`, 5.2). A real server's bound is unknown; the
/// client never asks for more.
pub const FETCH_MAX: u64 = 1_048_576;

/// What a new file is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileSpec {
    /// Its kind.
    pub kind: FileKind,
    /// Its length in bytes.
    pub length: u64,
    /// Its mode, raw.
    pub mode: u32,
    /// Its owner's protection id.
    pub owner: i32,
    /// The caller access mask of every context not given its own.
    pub access: u32,
    /// The anonymous access mask.
    pub anonymous: u32,
}

impl FileSpec {
    /// A file of `length` bytes, mode 0o644, owner 1, every mask 0.
    #[must_use]
    pub const fn file(length: u64) -> Self {
        Self {
            kind: FileKind::File,
            length,
            mode: 0o644,
            owner: 1,
            access: 0,
            anonymous: 0,
        }
    }
}

/// A volume's state on this server, and the abort each state answers
/// with (5.9).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VolumeState {
    /// On line.
    Online,
    /// Off line: code 106 (row R10).
    Offline,
    /// Moved to another server: code 111 (row R12).
    Moved,
}

/// A fault the model plants in its next bulk replies, so the client's
/// checks (5.4, point 10) can be tested.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BulkFault {
    /// One status record and one promise fewer than the files asked.
    DropLast,
    /// The item at this index has a kind outside the three known.
    UnclassifiedAt(usize),
}

/// A volume taken from one fileserver, to be put on another.
#[derive(Clone, Debug)]
pub struct MovedVolume {
    volume: VolumeId,
    data: Volume,
    files: Vec<(Fid, File)>,
}

#[derive(Clone, Debug)]
struct Volume {
    name: Vec<u8>,
    state: VolumeState,
    incarnation: u64,
}

#[derive(Clone, Debug)]
struct File {
    spec: FileSpec,
    data_version: u64,
    mtime: u64,
    access: BTreeMap<ContextId, u32>,
    writes: Vec<(u64, Vec<u8>)>,
}

/// One fileserver.
#[derive(Clone, Debug)]
pub struct FileServer {
    addr: Addr,
    volumes: BTreeMap<VolumeId, Volume>,
    files: BTreeMap<Fid, File>,
    refused: BTreeSet<ProcName>,
    busy: Option<Busy>,
    bulk_fault: Option<BulkFault>,
    promises: BTreeSet<(Fid, ClientId)>,
    owed: Vec<(ClientId, Fid)>,
    clock: u64,
}

impl FileServer {
    /// A fileserver at `addr` with no volume.
    #[must_use]
    pub const fn new(addr: Addr) -> Self {
        Self {
            addr,
            volumes: BTreeMap::new(),
            files: BTreeMap::new(),
            refused: BTreeSet::new(),
            busy: None,
            bulk_fault: None,
            promises: BTreeSet::new(),
            owed: Vec::new(),
            clock: 0,
        }
    }

    /// The server's address.
    #[must_use]
    pub const fn addr(&self) -> Addr {
        self.addr
    }

    /// Adds an on-line volume.
    pub fn add_volume(&mut self, volume: VolumeId, name: &[u8]) {
        self.volumes.insert(
            volume,
            Volume {
                name: name.to_vec(),
                state: VolumeState::Online,
                incarnation: 1,
            },
        );
    }

    /// Adds a file to its volume, at data version 1; `false` if the
    /// volume is not on this server.
    pub fn add_file(&mut self, fid: Fid, spec: FileSpec) -> bool {
        if !self.volumes.contains_key(&fid.volume) {
            return false;
        }
        self.files.insert(
            fid,
            File {
                spec,
                data_version: 1,
                mtime: 0,
                access: BTreeMap::new(),
                writes: Vec::new(),
            },
        );
        true
    }

    /// Gives `context` its own caller access mask on `fid`.
    pub fn set_access(&mut self, fid: Fid, context: ContextId, mask: u32) {
        if let Some(file) = self.files.get_mut(&fid) {
            file.access.insert(context, mask);
        }
    }

    /// Sets a volume's state; `false` if it is not on this server.
    pub fn set_volume_state(&mut self, volume: VolumeId, state: VolumeState) -> bool {
        self.volumes
            .get_mut(&volume)
            .map(|known| known.state = state)
            .is_some()
    }

    /// Whether the server holds `volume` on line.
    #[must_use]
    pub fn holds(&self, volume: VolumeId) -> bool {
        self.volume(volume).is_ok()
    }

    /// Takes `volume` and its files off this server, which from then on
    /// answers "moved" for it; its promises end with it. `None` if the
    /// server does not hold it on line.
    pub fn export_volume(&mut self, volume: VolumeId) -> Option<MovedVolume> {
        let known = self.volumes.get_mut(&volume)?;
        if known.state != VolumeState::Online {
            return None;
        }
        let data = known.clone();
        known.state = VolumeState::Moved;
        let fids: Vec<Fid> = self
            .files
            .keys()
            .filter(|fid| fid.volume == volume)
            .copied()
            .collect();
        let files = fids
            .into_iter()
            .filter_map(|fid| self.files.remove(&fid).map(|file| (fid, file)))
            .collect();
        self.promises.retain(|(fid, _)| fid.volume != volume);
        Some(MovedVolume {
            volume,
            data,
            files,
        })
    }

    /// Puts a moved volume, with its files, on this server.
    pub fn import_volume(&mut self, moved: MovedVolume) {
        self.volumes.insert(moved.volume, moved.data);
        self.files.extend(moved.files);
    }

    /// Makes the server answer `procedure` with the "unknown procedure"
    /// abort (5.9, row R6).
    pub fn refuse(&mut self, procedure: ProcName) {
        self.refused.insert(procedure);
    }

    /// Answers the next requests with a busy abort.
    pub fn busy(&mut self, busy: Busy) {
        self.busy = Some(busy);
    }

    /// Plants `fault` in every later bulk reply, or clears it.
    pub fn bulk_fault(&mut self, fault: Option<BulkFault>) {
        self.bulk_fault = fault;
    }

    /// The bytes of `fid` from `offset`, `length` of them, as the server
    /// holds them now; `None` for an unknown file or a range past its end.
    #[must_use]
    pub fn content(&self, fid: Fid, offset: u64, length: u64) -> Option<Vec<u8>> {
        let file = self.files.get(&fid)?;
        let end = offset.checked_add(length)?;
        if end > file.spec.length {
            return None;
        }
        let mut bytes: Vec<u8> = (offset..end).map(|at| base_byte(fid, at)).collect();
        for (start, written) in &file.writes {
            // The overlap of the write with the range, if any.
            let from = offset.max(*start);
            let written_end = start.checked_add(u64::try_from(written.len()).ok()?)?;
            let to = end.min(written_end);
            if from >= to {
                continue;
            }
            let source = usize::try_from(from - start).ok()?..usize::try_from(to - start).ok()?;
            let target = usize::try_from(from - offset).ok()?..usize::try_from(to - offset).ok()?;
            bytes.get_mut(target)?.copy_from_slice(written.get(source)?);
        }
        Some(bytes)
    }

    /// The clients holding a callback promise on `fid`.
    #[must_use]
    pub fn promise_holders(&self, fid: Fid) -> Vec<ClientId> {
        self.promises
            .range((fid, ClientId(0))..=(fid, ClientId(u32::MAX)))
            .map(|(_, client)| *client)
            .collect()
    }

    /// The breaks the server owes: each client whose promise a write by
    /// another client ended, with the file, in the order owed.
    #[must_use]
    pub fn owed_breaks(&self) -> &[(ClientId, Fid)] {
        &self.owed
    }

    /// The answer to `procedure` from `client` in `context`: a busy spell
    /// first, then a refusal, then the volume's state, then the files.
    pub fn serve(&mut self, client: ClientId, context: ContextId, procedure: &Procedure) -> Answer {
        if let Some(code) = Busy::take(&mut self.busy) {
            return Answer::Abort(code);
        }
        if self.refused.contains(&procedure.name()) {
            return Answer::Abort(UNKNOWN_PROCEDURE);
        }
        match self.answer(client, context, procedure) {
            Ok(results) => Answer::Result(results),
            Err(code) => Answer::Abort(code),
        }
    }

    fn answer(
        &mut self,
        client: ClientId,
        context: ContextId,
        procedure: &Procedure,
    ) -> Result<Results, i32> {
        match procedure {
            Procedure::FsFetchStatus { fid } => {
                let (status, sync) = self.status(*fid, context)?;
                Ok(Results::Status {
                    status,
                    promise: self.promise(client, *fid),
                    sync,
                })
            }
            Procedure::FsBulkStatus { fids } | Procedure::FsInlineBulkStatus { fids } => {
                self.bulk(client, context, fids)
            }
            Procedure::FsFetchData {
                fid,
                offset,
                length,
            } => self.fetch(
                client,
                context,
                *fid,
                u64::from(*offset),
                u64::from(*length),
            ),
            Procedure::FsFetchData64 {
                fid,
                offset,
                length,
            } => self.fetch(client, context, *fid, *offset, *length),
            Procedure::FsGetVolumeStatus { volume } => {
                let known = self.volume(*volume)?;
                let name = known.name.clone();
                let used: u64 = self
                    .files
                    .iter()
                    .filter(|(fid, _)| fid.volume == *volume)
                    .map(|(_, file)| file.spec.length.div_ceil(1024))
                    .fold(0, u64::saturating_add);
                Ok(Results::VolumeStatus(VolumeStatus {
                    volume: *volume,
                    name,
                    quota: 0,
                    blocks_in_use: used,
                    partition_free: 1 << 30,
                    partition_total: 1 << 31,
                    offline_message: Vec::new(),
                    motd: Vec::new(),
                }))
            }
            Procedure::PeerWrite { fid, offset, bytes } => self.write(client, *fid, *offset, bytes),
            Procedure::VlGetEntryByName { .. }
            | Procedure::VlGetEntryById { .. }
            | Procedure::VlGetAddrs { .. } => Err(UNKNOWN_PROCEDURE),
        }
    }

    /// The volume, if it is here and on line; else its abort code.
    fn volume(&self, volume: VolumeId) -> Result<&Volume, i32> {
        let known = self.volumes.get(&volume).ok_or(NOT_ON_SERVER)?;
        match known.state {
            VolumeState::Online => Ok(known),
            VolumeState::Offline => Err(VOLUME_OFFLINE),
            VolumeState::Moved => Err(VOLUME_MOVED),
        }
    }

    fn status(&self, fid: Fid, context: ContextId) -> Result<(Status, VolSync), i32> {
        let volume = self.volume(fid.volume)?;
        let file = self.files.get(&fid).ok_or(NO_VNODE)?;
        let status = Status {
            kind: file.spec.kind,
            link_count: 1,
            length: file.spec.length,
            data_version: file.data_version,
            author: file.spec.owner,
            owner: file.spec.owner,
            group: 0,
            caller_access: file
                .access
                .get(&context)
                .copied()
                .unwrap_or(file.spec.access),
            anonymous_access: file.spec.anonymous,
            mode: file.spec.mode,
            parent_vnode: 1,
            parent_unique: 1,
            client_mtime: file.mtime,
            server_mtime: file.mtime,
        };
        Ok((
            status,
            VolSync {
                incarnation: volume.incarnation,
            },
        ))
    }

    fn promise(&mut self, client: ClientId, fid: Fid) -> Promise {
        self.promises.insert((fid, client));
        Promise {
            lifetime: PROMISE_LIFETIME,
        }
    }

    fn bulk(&mut self, client: ClientId, context: ContextId, fids: &[Fid]) -> Result<Results, i32> {
        let mut statuses = Vec::with_capacity(fids.len());
        let mut promises = Vec::with_capacity(fids.len());
        let mut sync = None;
        for fid in fids {
            let (status, volume) = self.status(*fid, context)?;
            statuses.push(status);
            sync.get_or_insert(volume);
        }
        for fid in fids {
            promises.push(self.promise(client, *fid));
        }
        match self.bulk_fault {
            Some(BulkFault::DropLast) => {
                statuses.pop();
                promises.pop();
            }
            Some(BulkFault::UnclassifiedAt(index)) => {
                if let Some(status) = statuses.get_mut(index) {
                    status.kind = FileKind::Unclassified(0xff);
                }
            }
            None => {}
        }
        Ok(Results::Bulk {
            statuses,
            promises,
            // An empty list names no volume; the model answers it with
            // incarnation 0, and the client never sends one (5.5, point 2).
            sync: sync.unwrap_or(VolSync { incarnation: 0 }),
        })
    }

    /// A fetch of up to `length` bytes at `offset`: what the file holds
    /// from there, at most [`FETCH_MAX`], and none at or past its end.
    fn fetch(
        &mut self,
        client: ClientId,
        context: ContextId,
        fid: Fid,
        offset: u64,
        length: u64,
    ) -> Result<Results, i32> {
        let (status, sync) = self.status(fid, context)?;
        let left = status.length.saturating_sub(offset);
        let data = self
            .content(fid, offset, length.min(left).min(FETCH_MAX))
            .unwrap_or_default();
        Ok(Results::Data {
            data,
            status,
            promise: self.promise(client, fid),
            sync,
        })
    }

    /// The second client's write: the bytes land, the data version and
    /// the modification time move, and every other client's promise on
    /// the file ends with a break owed to it.
    fn write(
        &mut self,
        writer: ClientId,
        fid: Fid,
        offset: u64,
        bytes: &[u8],
    ) -> Result<Results, i32> {
        self.volume(fid.volume)?;
        let clock = self.clock.checked_add(1).ok_or(MARSHALLING)?;
        let file = self.files.get_mut(&fid).ok_or(NO_VNODE)?;
        let count = u64::try_from(bytes.len()).map_err(|_| MARSHALLING)?;
        let end = offset.checked_add(count).ok_or(MARSHALLING)?;
        let data_version = file.data_version.checked_add(1).ok_or(MARSHALLING)?;
        file.writes.push((offset, bytes.to_vec()));
        file.spec.length = file.spec.length.max(end);
        file.data_version = data_version;
        file.mtime = clock;
        self.clock = clock;
        for holder in self.promise_holders(fid) {
            if holder != writer {
                self.promises.remove(&(fid, holder));
                self.owed.push((holder, fid));
            }
        }
        Ok(Results::Wrote { data_version })
    }
}

/// The byte a file holds at `offset` before any write: one byte of a
/// mix of its id and the offset's eight-byte block, so two ranges of one
/// file, or one range of two files, differ, and a reply holding the wrong
/// range is seen.
fn base_byte(fid: Fid, offset: u64) -> u8 {
    let mut mixed = (u64::from(fid.volume.0) << 32 | u64::from(fid.vnode.0))
        ^ u64::from(fid.unique.0).rotate_left(17)
        ^ (offset / 8).wrapping_mul(0x9e37_79b9_7f4a_7c15);
    mixed = (mixed ^ (mixed >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    mixed ^= mixed >> 31;
    let index = usize::try_from(offset % 8).unwrap_or(0);
    mixed.to_le_bytes().get(index).copied().unwrap_or(0)
}
