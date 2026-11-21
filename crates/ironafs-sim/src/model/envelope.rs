// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The simulator's envelope: how a model's request and answer cross the
//! scheduler's byte-level server seam.
//!
//! This is the simulator's own encoding, not the AFS-3 wire: the read
//! specification marks every argument and result layout MISSING
//! (`docs/specs/vl-fs-read.md`, 5.1), so nothing here claims to be one.
//! A tag byte names each variant; integers are big-endian; a byte string
//! or a list is a 32-bit count, then its items. Decoding is strict and
//! total: every count is checked against the bytes left before anything
//! is allocated, and trailing bytes are an error.

use super::callback::{read_cb_proc, write_cb_proc};
use super::types::{
    Addr, Answer, ClientId, ContextId, Fid, FileKind, Procedure, Promise, Release, Request,
    Results, ServerUuid, Service, Site, SiteServer, Status, Uniquifier, VlEntry, Vnode, VolSync,
    VolumeId, VolumeIds, VolumeKind, VolumeStatus,
};

#[cfg(test)]
mod tests;

/// Why envelope bytes could not be decoded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnvelopeError {
    /// The bytes ended inside a value.
    Truncated,
    /// Bytes were left after the value.
    Trailing,
    /// A tag named no variant.
    Tag,
}

/// The envelope bytes of `request`.
#[must_use]
pub fn encode_request(request: &Request) -> Vec<u8> {
    let mut out = Writer::default();
    out.addr(request.to);
    out.u8(match request.service {
        Service::Vl => 0,
        Service::Fs => 1,
    });
    out.u32(request.client.0);
    out.u32(request.context.0);
    match &request.procedure {
        Procedure::VlGetEntryByName { name } => {
            out.u8(0);
            out.bytes(name);
        }
        Procedure::VlGetEntryById { volume, kind } => {
            out.u8(1);
            out.u32(volume.0);
            out.kind(*kind);
        }
        Procedure::VlGetAddrs { server } => {
            out.u8(2);
            out.raw(&server.0);
        }
        Procedure::FsFetchStatus { fid } => {
            out.u8(3);
            out.fid(*fid);
        }
        Procedure::FsBulkStatus { fids } => {
            out.u8(4);
            out.fids(fids);
        }
        Procedure::FsInlineBulkStatus { fids } => {
            out.u8(5);
            out.fids(fids);
        }
        Procedure::FsFetchData {
            fid,
            offset,
            length,
        } => {
            out.u8(6);
            out.fid(*fid);
            out.u32(*offset);
            out.u32(*length);
        }
        Procedure::FsFetchData64 {
            fid,
            offset,
            length,
        } => {
            out.u8(7);
            out.fid(*fid);
            out.u64(*offset);
            out.u64(*length);
        }
        Procedure::FsGetVolumeStatus { volume } => {
            out.u8(8);
            out.u32(volume.0);
        }
        Procedure::PeerWrite { fid, offset, bytes } => {
            out.u8(9);
            out.fid(*fid);
            out.u64(*offset);
            out.bytes(bytes);
        }
        Procedure::SimDeliverBreaks => out.u8(10),
        Procedure::SimReset { client } => {
            out.u8(11);
            out.u32(client.0);
        }
        Procedure::SimProbe { client } => {
            out.u8(12);
            out.u32(client.0);
        }
        Procedure::SimForge {
            from,
            to,
            procedure,
        } => {
            out.u8(13);
            out.addr(*from);
            out.u32(to.0);
            write_cb_proc(&mut out, procedure);
        }
    }
    out.0
}

/// The server a request's bytes are addressed to: their first four
/// bytes, which every request starts with, read before anything else so
/// that bytes to a server that is not there are dropped unread.
#[must_use]
pub fn destination(bytes: &[u8]) -> Option<Addr> {
    Reader(bytes).addr().ok()
}

/// The request `bytes` encode.
///
/// # Errors
///
/// An [`EnvelopeError`] if `bytes` are not exactly one request.
pub fn decode_request(bytes: &[u8]) -> Result<Request, EnvelopeError> {
    let mut input = Reader(bytes);
    let to = input.addr()?;
    let service = match input.u8()? {
        0 => Service::Vl,
        1 => Service::Fs,
        _ => return Err(EnvelopeError::Tag),
    };
    let client = ClientId(input.u32()?);
    let context = ContextId(input.u32()?);
    let procedure = match input.u8()? {
        0 => Procedure::VlGetEntryByName {
            name: input.bytes()?,
        },
        1 => Procedure::VlGetEntryById {
            volume: VolumeId(input.u32()?),
            kind: input.kind()?,
        },
        2 => Procedure::VlGetAddrs {
            server: ServerUuid(input.array()?),
        },
        3 => Procedure::FsFetchStatus { fid: input.fid()? },
        4 => Procedure::FsBulkStatus {
            fids: input.fids()?,
        },
        5 => Procedure::FsInlineBulkStatus {
            fids: input.fids()?,
        },
        6 => Procedure::FsFetchData {
            fid: input.fid()?,
            offset: input.u32()?,
            length: input.u32()?,
        },
        7 => Procedure::FsFetchData64 {
            fid: input.fid()?,
            offset: input.u64()?,
            length: input.u64()?,
        },
        8 => Procedure::FsGetVolumeStatus {
            volume: VolumeId(input.u32()?),
        },
        9 => Procedure::PeerWrite {
            fid: input.fid()?,
            offset: input.u64()?,
            bytes: input.bytes()?,
        },
        10 => Procedure::SimDeliverBreaks,
        11 => Procedure::SimReset {
            client: ClientId(input.u32()?),
        },
        12 => Procedure::SimProbe {
            client: ClientId(input.u32()?),
        },
        13 => Procedure::SimForge {
            from: input.addr()?,
            to: ClientId(input.u32()?),
            procedure: read_cb_proc(&mut input)?,
        },
        _ => return Err(EnvelopeError::Tag),
    };
    input.end()?;
    Ok(Request {
        to,
        service,
        client,
        context,
        procedure,
    })
}

/// The envelope bytes of `answer`.
#[must_use]
pub fn encode_answer(answer: &Answer) -> Vec<u8> {
    let mut out = Writer::default();
    let results = match answer {
        Answer::Abort(code) => {
            out.u8(0);
            out.i32(*code);
            return out.0;
        }
        Answer::Result(results) => results,
    };
    out.u8(1);
    match results {
        Results::VlEntry(entry) => {
            out.u8(0);
            out.bytes(&entry.name);
            for id in [entry.ids.read_write, entry.ids.read_only, entry.ids.backup] {
                out.u32(id.map_or(0, |id| id.0));
            }
            out.count(entry.sites.len());
            for site in &entry.sites {
                out.site(site);
            }
        }
        Results::Addrs(addrs) => {
            out.u8(1);
            out.count(addrs.len());
            for addr in addrs {
                out.addr(*addr);
            }
        }
        Results::Status {
            status,
            promise,
            sync,
        } => {
            out.u8(2);
            out.status(status);
            out.u32(promise.lifetime);
            out.u64(sync.incarnation);
        }
        Results::Bulk {
            statuses,
            promises,
            sync,
        } => {
            out.u8(3);
            out.count(statuses.len());
            for status in statuses {
                out.status(status);
            }
            out.count(promises.len());
            for promise in promises {
                out.u32(promise.lifetime);
            }
            out.u64(sync.incarnation);
        }
        Results::Data {
            data,
            status,
            promise,
            sync,
        } => {
            out.u8(4);
            out.bytes(data);
            out.status(status);
            out.u32(promise.lifetime);
            out.u64(sync.incarnation);
        }
        Results::VolumeStatus(volume) => {
            out.u8(5);
            out.u32(volume.volume.0);
            out.bytes(&volume.name);
            for count in [
                volume.quota,
                volume.blocks_in_use,
                volume.partition_free,
                volume.partition_total,
            ] {
                out.u64(count);
            }
            out.bytes(&volume.offline_message);
            out.bytes(&volume.motd);
        }
        Results::Wrote { data_version } => {
            out.u8(6);
            out.u64(*data_version);
        }
        Results::Started { pushes } => {
            out.u8(7);
            out.u32(*pushes);
        }
    }
    out.0
}

/// The answer `bytes` encode.
///
/// # Errors
///
/// An [`EnvelopeError`] if `bytes` are not exactly one answer.
pub fn decode_answer(bytes: &[u8]) -> Result<Answer, EnvelopeError> {
    let mut input = Reader(bytes);
    let answer = match input.u8()? {
        0 => Answer::Abort(input.i32()?),
        1 => Answer::Result(input.results()?),
        _ => return Err(EnvelopeError::Tag),
    };
    input.end()?;
    Ok(answer)
}

/// The fewest bytes a site encodes to: a tag, an address, a partition,
/// a version and a release state.
const SITE_MIN: usize = 11;

/// The bytes every status record encodes to.
const STATUS_MIN: usize = 70;

/// Appends envelope values to a buffer; shared with the callback
/// envelope.
#[derive(Default)]
pub(super) struct Writer(Vec<u8>);

impl Writer {
    pub(super) fn into_bytes(self) -> Vec<u8> {
        self.0
    }

    pub(super) fn u8(&mut self, value: u8) {
        self.0.push(value);
    }

    pub(super) fn u32(&mut self, value: u32) {
        self.0.extend_from_slice(&value.to_be_bytes());
    }

    pub(super) fn i32(&mut self, value: i32) {
        self.0.extend_from_slice(&value.to_be_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.0.extend_from_slice(&value.to_be_bytes());
    }

    fn raw(&mut self, bytes: &[u8]) {
        self.0.extend_from_slice(bytes);
    }

    /// A count; the models never build a list of 2^32 items, so the
    /// saturation is never reached.
    fn count(&mut self, count: usize) {
        self.u32(u32::try_from(count).unwrap_or(u32::MAX));
    }

    fn bytes(&mut self, bytes: &[u8]) {
        self.count(bytes.len());
        self.raw(bytes);
    }

    pub(super) fn addr(&mut self, addr: Addr) {
        self.raw(&addr.0);
    }

    fn fid(&mut self, fid: Fid) {
        self.u32(fid.volume.0);
        self.u32(fid.vnode.0);
        self.u32(fid.unique.0);
    }

    pub(super) fn fids(&mut self, fids: &[Fid]) {
        self.count(fids.len());
        for fid in fids {
            self.fid(*fid);
        }
    }

    fn kind(&mut self, kind: VolumeKind) {
        self.u8(match kind {
            VolumeKind::ReadWrite => 0,
            VolumeKind::ReadOnly => 1,
            VolumeKind::Backup => 2,
        });
    }

    fn site(&mut self, site: &Site) {
        match site.server {
            SiteServer::Addr(addr) => {
                self.u8(0);
                self.addr(addr);
            }
            SiteServer::Uuid(uuid) => {
                self.u8(1);
                self.raw(&uuid.0);
            }
        }
        self.u32(site.partition);
        self.kind(site.holds);
        self.u8(match site.release {
            Release::Released => 0,
            Release::NotReleased => 1,
            Release::Old => 2,
            Release::New => 3,
        });
    }

    fn status(&mut self, status: &Status) {
        match status.kind {
            FileKind::File => self.raw(&[0, 0]),
            FileKind::Directory => self.raw(&[1, 0]),
            FileKind::Symlink => self.raw(&[2, 0]),
            FileKind::Unclassified(value) => self.raw(&[3, value]),
        }
        self.u32(status.link_count);
        self.u64(status.length);
        self.u64(status.data_version);
        self.i32(status.author);
        self.i32(status.owner);
        self.i32(status.group);
        self.u32(status.caller_access);
        self.u32(status.anonymous_access);
        self.u32(status.mode);
        self.u32(status.parent_vnode);
        self.u32(status.parent_unique);
        self.u64(status.client_mtime);
        self.u64(status.server_mtime);
    }
}

/// Reads envelope values from the front of a byte slice; shared with
/// the callback envelope.
pub(super) struct Reader<'a>(&'a [u8]);

impl<'a> Reader<'a> {
    pub(super) const fn new(bytes: &'a [u8]) -> Self {
        Self(bytes)
    }

    fn take(&mut self, count: usize) -> Result<&[u8], EnvelopeError> {
        let Some((head, rest)) = self.0.split_at_checked(count) else {
            return Err(EnvelopeError::Truncated);
        };
        self.0 = rest;
        Ok(head)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], EnvelopeError> {
        self.take(N)?
            .try_into()
            .map_err(|_| EnvelopeError::Truncated)
    }

    pub(super) fn end(&self) -> Result<(), EnvelopeError> {
        if self.0.is_empty() {
            Ok(())
        } else {
            Err(EnvelopeError::Trailing)
        }
    }

    pub(super) fn u8(&mut self) -> Result<u8, EnvelopeError> {
        let [value] = self.array()?;
        Ok(value)
    }

    pub(super) fn u32(&mut self) -> Result<u32, EnvelopeError> {
        Ok(u32::from_be_bytes(self.array()?))
    }

    pub(super) fn i32(&mut self) -> Result<i32, EnvelopeError> {
        Ok(i32::from_be_bytes(self.array()?))
    }

    fn u64(&mut self) -> Result<u64, EnvelopeError> {
        Ok(u64::from_be_bytes(self.array()?))
    }

    /// A count of items each at least `item` bytes long, checked against
    /// the bytes left before any allocation.
    fn count(&mut self, item: usize) -> Result<usize, EnvelopeError> {
        let count = usize::try_from(self.u32()?).map_err(|_| EnvelopeError::Truncated)?;
        match count.checked_mul(item) {
            Some(needed) if needed <= self.0.len() => Ok(count),
            _ => Err(EnvelopeError::Truncated),
        }
    }

    fn bytes(&mut self) -> Result<Vec<u8>, EnvelopeError> {
        let count = self.count(1)?;
        Ok(self.take(count)?.to_vec())
    }

    pub(super) fn addr(&mut self) -> Result<Addr, EnvelopeError> {
        Ok(Addr(self.array()?))
    }

    fn fid(&mut self) -> Result<Fid, EnvelopeError> {
        Ok(Fid {
            volume: VolumeId(self.u32()?),
            vnode: Vnode(self.u32()?),
            unique: Uniquifier(self.u32()?),
        })
    }

    pub(super) fn fids(&mut self) -> Result<Vec<Fid>, EnvelopeError> {
        let count = self.count(12)?;
        (0..count).map(|_| self.fid()).collect()
    }

    fn kind(&mut self) -> Result<VolumeKind, EnvelopeError> {
        match self.u8()? {
            0 => Ok(VolumeKind::ReadWrite),
            1 => Ok(VolumeKind::ReadOnly),
            2 => Ok(VolumeKind::Backup),
            _ => Err(EnvelopeError::Tag),
        }
    }

    fn id(&mut self) -> Result<Option<VolumeId>, EnvelopeError> {
        let id = self.u32()?;
        Ok((id != 0).then_some(VolumeId(id)))
    }

    fn site(&mut self) -> Result<Site, EnvelopeError> {
        let server = match self.u8()? {
            0 => SiteServer::Addr(self.addr()?),
            1 => SiteServer::Uuid(ServerUuid(self.array()?)),
            _ => return Err(EnvelopeError::Tag),
        };
        Ok(Site {
            server,
            partition: self.u32()?,
            holds: self.kind()?,
            release: match self.u8()? {
                0 => Release::Released,
                1 => Release::NotReleased,
                2 => Release::Old,
                3 => Release::New,
                _ => return Err(EnvelopeError::Tag),
            },
        })
    }

    fn status(&mut self) -> Result<Status, EnvelopeError> {
        let kind = match self.array()? {
            [0, 0] => FileKind::File,
            [1, 0] => FileKind::Directory,
            [2, 0] => FileKind::Symlink,
            [3, value] => FileKind::Unclassified(value),
            _ => return Err(EnvelopeError::Tag),
        };
        Ok(Status {
            kind,
            link_count: self.u32()?,
            length: self.u64()?,
            data_version: self.u64()?,
            author: self.i32()?,
            owner: self.i32()?,
            group: self.i32()?,
            caller_access: self.u32()?,
            anonymous_access: self.u32()?,
            mode: self.u32()?,
            parent_vnode: self.u32()?,
            parent_unique: self.u32()?,
            client_mtime: self.u64()?,
            server_mtime: self.u64()?,
        })
    }

    fn promise(&mut self) -> Result<Promise, EnvelopeError> {
        Ok(Promise {
            lifetime: self.u32()?,
        })
    }

    fn sync(&mut self) -> Result<VolSync, EnvelopeError> {
        Ok(VolSync {
            incarnation: self.u64()?,
        })
    }

    fn results(&mut self) -> Result<Results, EnvelopeError> {
        Ok(match self.u8()? {
            0 => {
                let name = self.bytes()?;
                let ids = VolumeIds {
                    read_write: self.id()?,
                    read_only: self.id()?,
                    backup: self.id()?,
                };
                let count = self.count(SITE_MIN)?;
                let sites = (0..count).map(|_| self.site()).collect::<Result<_, _>>()?;
                Results::VlEntry(VlEntry { name, ids, sites })
            }
            1 => {
                let count = self.count(4)?;
                Results::Addrs((0..count).map(|_| self.addr()).collect::<Result<_, _>>()?)
            }
            2 => Results::Status {
                status: self.status()?,
                promise: self.promise()?,
                sync: self.sync()?,
            },
            3 => {
                let count = self.count(STATUS_MIN)?;
                let statuses = (0..count)
                    .map(|_| self.status())
                    .collect::<Result<_, _>>()?;
                let count = self.count(4)?;
                let promises = (0..count)
                    .map(|_| self.promise())
                    .collect::<Result<_, _>>()?;
                Results::Bulk {
                    statuses,
                    promises,
                    sync: self.sync()?,
                }
            }
            4 => Results::Data {
                data: self.bytes()?,
                status: self.status()?,
                promise: self.promise()?,
                sync: self.sync()?,
            },
            5 => Results::VolumeStatus(VolumeStatus {
                volume: VolumeId(self.u32()?),
                name: self.bytes()?,
                quota: self.u64()?,
                blocks_in_use: self.u64()?,
                partition_free: self.u64()?,
                partition_total: self.u64()?,
                offline_message: self.bytes()?,
                motd: self.bytes()?,
            }),
            6 => Results::Wrote {
                data_version: self.u64()?,
            },
            7 => Results::Started {
                pushes: self.u32()?,
            },
            _ => return Err(EnvelopeError::Tag),
        })
    }
}
