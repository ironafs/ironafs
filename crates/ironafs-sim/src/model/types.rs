// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The values the models' procedures carry, by the roles the
//! specification names (`docs/specs/vl-fs-read.md`, 5.1.3 and 5.1.4):
//! typed fields, never wire words. Access masks are carried raw and
//! never interpreted by a model (5.7, point 2: no bit position is known).

/// A volume id (ADR 0004, N38, point 2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VolumeId(pub u32);

/// A vnode number.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Vnode(pub u32);

/// A uniquifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Uniquifier(pub u32);

/// A file id: volume, vnode and uniquifier (5.1.3, point 1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Fid {
    /// The volume.
    pub volume: VolumeId,
    /// The vnode.
    pub vnode: Vnode,
    /// The uniquifier.
    pub unique: Uniquifier,
}

impl Fid {
    /// The file id of `volume`, `vnode` and `unique`.
    #[must_use]
    pub const fn new(volume: u32, vnode: u32, unique: u32) -> Self {
        Self {
            volume: VolumeId(volume),
            vnode: Vnode(vnode),
            unique: Uniquifier(unique),
        }
    }
}

/// A server's IPv4 address (5.1.3, point 7), as its four octets.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Addr(pub [u8; 4]);

/// The Rx service a request is for (5.1.1, point 5).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Service {
    /// The volume location service.
    Vl,
    /// The file service.
    Fs,
}

/// A simulated client host: the one under test, or another that writes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ClientId(pub u32);

/// A credential context on a client: the caller a status record's
/// caller access mask is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContextId(pub u32);

/// A version of a volume (5.1.4, point 2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum VolumeKind {
    /// The read/write volume.
    ReadWrite,
    /// A read-only replica.
    ReadOnly,
    /// The backup clone.
    Backup,
}

/// An object's kind (5.1.3, point 2, `file_type`). `Unclassified` is a
/// value outside the three known kinds, which a model returns only when
/// a test asks it to, so the client's rejection can be tested.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FileKind {
    /// A file.
    File,
    /// A directory.
    Directory,
    /// A symbolic link.
    Symlink,
    /// No kind the specification names.
    Unclassified(u8),
}

/// A fileserver's identity, the 16 octets a UUID denotes (5.1.3, point 6).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ServerUuid(pub [u8; 16]);

/// How a site names its server (5.1.3, point 5, `site.server`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SiteServer {
    /// By address.
    Addr(Addr),
    /// By identity, resolved through `vl.get_addrs`.
    Uuid(ServerUuid),
}

/// The release state of a read-only site (5.1.3, point 5, `site.flags`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Release {
    /// Released: current.
    Released,
    /// Not released.
    NotReleased,
    /// Of an old release.
    Old,
    /// Of a new release.
    New,
}

/// One site of a volume (5.1.3, point 5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Site {
    /// The server.
    pub server: SiteServer,
    /// The partition.
    pub partition: u32,
    /// The version the site holds.
    pub holds: VolumeKind,
    /// Its release state.
    pub release: Release,
}

/// The ids of a volume's versions; `None` for a version that does not
/// exist (5.4, point 11: id 0 means absent).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VolumeIds {
    /// The read/write volume.
    pub read_write: Option<VolumeId>,
    /// The read-only replica.
    pub read_only: Option<VolumeId>,
    /// The backup clone.
    pub backup: Option<VolumeId>,
}

impl VolumeIds {
    /// The id of version `kind`, if it exists.
    #[must_use]
    pub const fn of(&self, kind: VolumeKind) -> Option<VolumeId> {
        match kind {
            VolumeKind::ReadWrite => self.read_write,
            VolumeKind::ReadOnly => self.read_only,
            VolumeKind::Backup => self.backup,
        }
    }
}

/// A VL entry (5.1.3, point 5).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VlEntry {
    /// The volume's base name.
    pub name: Vec<u8>,
    /// The ids of its versions.
    pub ids: VolumeIds,
    /// Where each version lives.
    pub sites: Vec<Site>,
}

/// A status record (5.1.3, point 2), by role.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Status {
    /// The object's kind.
    pub kind: FileKind,
    /// The link count.
    pub link_count: u32,
    /// The object's size in bytes.
    pub length: u64,
    /// The version of the bytes.
    pub data_version: u64,
    /// The author's protection id.
    pub author: i32,
    /// The owner's protection id.
    pub owner: i32,
    /// The group's protection id.
    pub group: i32,
    /// The caller's access mask, raw.
    pub caller_access: u32,
    /// The anonymous access mask, raw.
    pub anonymous_access: u32,
    /// The mode, raw.
    pub mode: u32,
    /// The parent's vnode.
    pub parent_vnode: u32,
    /// The parent's uniquifier.
    pub parent_unique: u32,
    /// The client modification time, in the model's seconds.
    pub client_mtime: u64,
    /// The server modification time, in the model's seconds.
    pub server_mtime: u64,
}

/// A callback promise (5.1.3, point 3). Its wire fields are MISSING; the
/// model's promise is a lifetime in seconds, 0 for none granted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Promise {
    /// The promise's lifetime, in seconds.
    pub lifetime: u32,
}

/// A volume sync record (5.1.3, point 4). Its wire fields are MISSING;
/// the model's record is the volume's incarnation, which #78's logic
/// reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VolSync {
    /// The volume's incarnation.
    pub incarnation: u64,
}

/// A volume's statistics (5.1.4, point 7).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VolumeStatus {
    /// The volume asked for.
    pub volume: VolumeId,
    /// The volume's name.
    pub name: Vec<u8>,
    /// The quota, in blocks.
    pub quota: u64,
    /// The blocks in use.
    pub blocks_in_use: u64,
    /// The partition's free blocks.
    pub partition_free: u64,
    /// The partition's total blocks.
    pub partition_total: u64,
    /// The offline message.
    pub offline_message: Vec<u8>,
    /// The message of the day.
    pub motd: Vec<u8>,
}

/// The procedures the models serve, by their IronAFS names (5.1.2), and
/// the simulator's own write by a second client.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Procedure {
    /// `vl.get_entry_by_name`.
    VlGetEntryByName {
        /// The volume name.
        name: Vec<u8>,
    },
    /// `vl.get_entry_by_id`.
    VlGetEntryById {
        /// The volume id.
        volume: VolumeId,
        /// The version it names.
        kind: VolumeKind,
    },
    /// `vl.get_addrs`.
    VlGetAddrs {
        /// The server's identity.
        server: ServerUuid,
    },
    /// `fs.fetch_status`.
    FsFetchStatus {
        /// The file.
        fid: Fid,
    },
    /// `fs.bulk_status`.
    FsBulkStatus {
        /// The files, in one volume.
        fids: Vec<Fid>,
    },
    /// `fs.inline_bulk_status`.
    FsInlineBulkStatus {
        /// The files, in one volume.
        fids: Vec<Fid>,
    },
    /// `fs.fetch_data`, with 32-bit offset and length.
    FsFetchData {
        /// The file.
        fid: Fid,
        /// The offset.
        offset: u32,
        /// The length asked.
        length: u32,
    },
    /// `fs.fetch_data64`, with 64-bit offset and length.
    FsFetchData64 {
        /// The file.
        fid: Fid,
        /// The offset.
        offset: u64,
        /// The length asked.
        length: u64,
    },
    /// `fs.get_volume_status`.
    FsGetVolumeStatus {
        /// The volume.
        volume: VolumeId,
    },
    /// A second simulated client's write: `bytes` at `offset`. Not an
    /// AFS-3 procedure (writes are outside the read specification); the
    /// simulator's own, so a competing writer changes a file's data
    /// version the way the client under test must see.
    PeerWrite {
        /// The file.
        fid: Fid,
        /// Where the bytes go.
        offset: u64,
        /// The bytes.
        bytes: Vec<u8>,
    },
}

/// A procedure's name, without its arguments.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ProcName {
    /// `vl.get_entry_by_name`.
    VlGetEntryByName,
    /// `vl.get_entry_by_id`.
    VlGetEntryById,
    /// `vl.get_addrs`.
    VlGetAddrs,
    /// `fs.fetch_status`.
    FsFetchStatus,
    /// `fs.bulk_status`.
    FsBulkStatus,
    /// `fs.inline_bulk_status`.
    FsInlineBulkStatus,
    /// `fs.fetch_data`.
    FsFetchData,
    /// `fs.fetch_data64`.
    FsFetchData64,
    /// `fs.get_volume_status`.
    FsGetVolumeStatus,
    /// The simulator's peer write.
    PeerWrite,
}

impl ProcName {
    /// The IronAFS name, as the trace prints it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::VlGetEntryByName => "vl.get_entry_by_name",
            Self::VlGetEntryById => "vl.get_entry_by_id",
            Self::VlGetAddrs => "vl.get_addrs",
            Self::FsFetchStatus => "fs.fetch_status",
            Self::FsBulkStatus => "fs.bulk_status",
            Self::FsInlineBulkStatus => "fs.inline_bulk_status",
            Self::FsFetchData => "fs.fetch_data",
            Self::FsFetchData64 => "fs.fetch_data64",
            Self::FsGetVolumeStatus => "fs.get_volume_status",
            Self::PeerWrite => "sim.peer_write",
        }
    }
}

impl Procedure {
    /// The procedure's name.
    #[must_use]
    pub const fn name(&self) -> ProcName {
        match self {
            Self::VlGetEntryByName { .. } => ProcName::VlGetEntryByName,
            Self::VlGetEntryById { .. } => ProcName::VlGetEntryById,
            Self::VlGetAddrs { .. } => ProcName::VlGetAddrs,
            Self::FsFetchStatus { .. } => ProcName::FsFetchStatus,
            Self::FsBulkStatus { .. } => ProcName::FsBulkStatus,
            Self::FsInlineBulkStatus { .. } => ProcName::FsInlineBulkStatus,
            Self::FsFetchData { .. } => ProcName::FsFetchData,
            Self::FsFetchData64 { .. } => ProcName::FsFetchData64,
            Self::FsGetVolumeStatus { .. } => ProcName::FsGetVolumeStatus,
            Self::PeerWrite { .. } => ProcName::PeerWrite,
        }
    }
}

/// One request: who sends it, to which server and service, in which
/// context, and the procedure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Request {
    /// The server's address.
    pub to: Addr,
    /// The service.
    pub service: Service,
    /// The calling client host.
    pub client: ClientId,
    /// The calling context.
    pub context: ContextId,
    /// The procedure and its arguments.
    pub procedure: Procedure,
}

/// A procedure's results.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Results {
    /// A VL entry, for either lookup.
    VlEntry(VlEntry),
    /// A server's addresses, for `vl.get_addrs`.
    Addrs(Vec<Addr>),
    /// One file's status, for `fs.fetch_status`.
    Status {
        /// The status record.
        status: Status,
        /// The callback promise.
        promise: Promise,
        /// The volume sync record.
        sync: VolSync,
    },
    /// The statuses of several files, for either bulk call.
    Bulk {
        /// One status record per file asked, in order.
        statuses: Vec<Status>,
        /// One callback promise per file asked, in order.
        promises: Vec<Promise>,
        /// The volume sync record.
        sync: VolSync,
    },
    /// A byte range and the file's status, for either fetch.
    Data {
        /// The bytes.
        data: Vec<u8>,
        /// The status record.
        status: Status,
        /// The callback promise.
        promise: Promise,
        /// The volume sync record.
        sync: VolSync,
    },
    /// A volume's statistics.
    VolumeStatus(VolumeStatus),
    /// A peer write's new data version.
    Wrote {
        /// The file's data version after the write.
        data_version: u64,
    },
}

/// A server's answer: results, or an abort with a code (5.1.1, point 3).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Answer {
    /// The procedure's results.
    Result(Results),
    /// An abort code, classified by the procedure and service (5.9).
    Abort(i32),
}
