// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The event catalogue: the types of an event definition, and the entries
//! (`docs/specs/events.md` §5.1; ADR 0004, O1, point 2, and O2, point 6).
//!
//! The entries are written by hand in `entries`. `cargo xtask events`
//! will generate that file from the catalogue's source and check it in.
// TODO(#94): generate `entries` with `cargo xtask events`.

mod entries;

#[cfg(test)]
mod tests;

pub use entries::{
    ALL, AUDIT_GAP, EVENTS_LOST, EVENTS_OVER_SIZE, EVENTS_SUPPRESSED, SERVER_REACHABLE,
    SERVER_UNREACHABLE, VOLUME_OFFLINE,
};

/// The catalogue's own version (ADR 0004, O2, point 6).
pub const CATALOGUE_VERSION: u16 = 1;

/// The most fields one event has (`docs/specs/events.md` §5.1, point 1).
pub const MAX_FIELDS: usize = 8;

/// The stream an event belongs to. Each stream has its own id space, and
/// only the daemon emits audit (ADR 0004, O9, point 1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Stream {
    /// Every event that is not audit.
    Diagnostic,
    /// The audit stream.
    Audit,
}

impl Stream {
    /// The stream's name in encodings: `diagnostic` or `audit`.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Diagnostic => "diagnostic",
            Self::Audit => "audit",
        }
    }
}

/// An event's severity: RFC 5424, section 6.2.1, with the same numbers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    /// 0: the system is unusable.
    Emergency,
    /// 1: action must be taken immediately.
    Alert,
    /// 2: a critical condition.
    Critical,
    /// 3: an error.
    Error,
    /// 4: a warning.
    Warning,
    /// 5: normal but significant.
    Notice,
    /// 6: informational.
    Informational,
    /// 7: debug.
    Debug,
}

impl Severity {
    /// The severity's number, 0 to 7, as RFC 5424 and journald's
    /// `PRIORITY` use it.
    #[must_use]
    pub const fn number(self) -> u8 {
        match self {
            Self::Emergency => 0,
            Self::Alert => 1,
            Self::Critical => 2,
            Self::Error => 3,
            Self::Warning => 4,
            Self::Notice => 5,
            Self::Informational => 6,
            Self::Debug => 7,
        }
    }
}

/// Who an event is for (ADR 0004, O1, point 2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Audience {
    /// Administrators.
    Admin,
    /// Operations: the client's own working.
    Operational,
    /// The audit stream.
    Audit,
    /// Debugging, with no promise to collectors (ADR 0004, O2, point 4).
    Debug,
}

impl Audience {
    /// The audience's name in encodings.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Admin => "admin",
            Self::Operational => "operational",
            Self::Audit => "audit",
            Self::Debug => "debug",
        }
    }
}

/// The subsystem an event comes from: a closed list, which bounds the rate
/// limiter's state (`docs/specs/events.md` §5.1, point 3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Subsystem {
    /// The event machinery itself: losses and summaries.
    Events,
    /// The daemon's own life.
    Daemon,
    /// The Rx transport.
    Rx,
    /// Volume location and file servers.
    Server,
    /// Volumes.
    Volume,
    /// The cache.
    Cache,
    /// Configuration and policy.
    Config,
    /// An operating-system layer.
    Adapter,
}

impl Subsystem {
    /// Every subsystem, in order.
    pub const ALL: [Self; 8] = [
        Self::Events,
        Self::Daemon,
        Self::Rx,
        Self::Server,
        Self::Volume,
        Self::Cache,
        Self::Config,
        Self::Adapter,
    ];

    /// How many subsystems there are.
    pub const COUNT: usize = Self::ALL.len();

    /// The subsystem's position in [`Subsystem::ALL`].
    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::Events => 0,
            Self::Daemon => 1,
            Self::Rx => 2,
            Self::Server => 3,
            Self::Volume => 4,
            Self::Cache => 5,
            Self::Config => 6,
            Self::Adapter => 7,
        }
    }

    /// The subsystem's name in encodings.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Events => "events",
            Self::Daemon => "daemon",
            Self::Rx => "rx",
            Self::Server => "server",
            Self::Volume => "volume",
            Self::Cache => "cache",
            Self::Config => "config",
            Self::Adapter => "adapter",
        }
    }
}

/// A kind of component that emits events (ADR 0004, O3, point 1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Role {
    /// The daemon.
    Daemon,
    /// An operating-system layer: the FUSE layer or the FSKit extension.
    Adapter,
    /// The SMB front end.
    SmbFrontEnd,
    /// A Windows helper process.
    WindowsHelper,
    /// The `afs` command line.
    Cli,
    /// The login service.
    LoginService,
}

impl Role {
    const fn bit(self) -> u8 {
        match self {
            Self::Daemon => 1,
            Self::Adapter => 1 << 1,
            Self::SmbFrontEnd => 1 << 2,
            Self::WindowsHelper => 1 << 3,
            Self::Cli => 1 << 4,
            Self::LoginService => 1 << 5,
        }
    }
}

/// A set of producer roles.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Roles(u8);

impl Roles {
    /// Only the daemon: every audit event (ADR 0004, O9, point 1).
    pub const DAEMON: Self = Self(Role::Daemon.bit());

    /// Every role.
    pub const ALL: Self = Self(
        Role::Daemon.bit()
            | Role::Adapter.bit()
            | Role::SmbFrontEnd.bit()
            | Role::WindowsHelper.bit()
            | Role::Cli.bit()
            | Role::LoginService.bit(),
    );

    /// Whether `role` is in the set.
    #[must_use]
    pub const fn contains(self, role: Role) -> bool {
        self.0 & role.bit() != 0
    }

    /// Whether the set is empty.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

/// What a field's value may reveal, and so how redaction treats it
/// (`docs/specs/events.md` §5.4). No class holds a secret.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DataClass {
    /// Counts, states and IronAFS's own static text.
    Public,
    /// Paths, file names, native file ids and volume numbers: omitted from
    /// routine logs (ADR 0004, O6, point 1).
    Identifier,
    /// Text a server sent: always escaped (ADR 0004, O5, point 3).
    ServerText,
    /// An opaque reference (ADR 0004, O6, point 2).
    Reference,
}

/// A field's value type (`docs/specs/events.md` §5.2, point 1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FieldKind {
    /// An unsigned integer, up to 64 bits.
    Unsigned,
    /// A signed integer, up to 64 bits.
    Signed,
    /// A boolean.
    Bool,
    /// Text written in IronAFS's own source.
    Static,
    /// Text a server sent ([`ServerText`](crate::ServerText)).
    ServerText,
    /// An opaque reference ([`Reference`](crate::Reference)).
    Reference,
}

impl FieldKind {
    /// Whether a field of this kind may have the data class `class`
    /// (`docs/specs/events.md` §5.4, point 2).
    #[must_use]
    pub const fn allows(self, class: DataClass) -> bool {
        matches!(
            (self, class),
            (
                Self::Unsigned | Self::Signed,
                DataClass::Public | DataClass::Identifier
            ) | (Self::Bool | Self::Static, DataClass::Public)
                | (
                    Self::ServerText,
                    DataClass::ServerText | DataClass::Identifier
                )
                | (Self::Reference, DataClass::Reference)
        )
    }
}

/// One field of an event definition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FieldSpec {
    /// The field's name: lower-case ASCII letters, digits and `_`, at most
    /// 24 bytes.
    pub name: &'static str,
    /// The value's type.
    pub kind: FieldKind,
    /// What the value may reveal.
    pub class: DataClass,
    /// Whether every event of this definition carries it.
    pub required: bool,
}

/// One catalogue entry (ADR 0004, O1, point 2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EventDef {
    /// The stream; ids are unique within it.
    pub stream: Stream,
    /// The id, never reused (ADR 0004, O2, point 1).
    pub id: u16,
    /// The version, from 1.
    pub version: u8,
    /// The name: lower-case ASCII letters, digits, `.` and `_`, at most 32
    /// bytes, unique in the catalogue.
    pub name: &'static str,
    /// The severity.
    pub severity: Severity,
    /// Who the event is for.
    pub audience: Audience,
    /// Where it comes from.
    pub subsystem: Subsystem,
    /// Its fields, at most [`MAX_FIELDS`].
    pub fields: &'static [FieldSpec],
    /// The message, with `{name}` placeholders for fields.
    pub template: &'static str,
    /// What the event means.
    pub meaning: &'static str,
    /// What an operator does about it.
    pub remedy: &'static str,
    /// The roles allowed to emit it.
    pub producers: Roles,
}

impl EventDef {
    /// The position of the field called `name`, if the event has one.
    #[must_use]
    pub fn field_index(&self, name: &str) -> Option<usize> {
        self.fields.iter().position(|field| field.name == name)
    }
}

/// The definition with this stream and id, if the catalogue has one.
#[must_use]
pub fn find(stream: Stream, id: u16) -> Option<&'static EventDef> {
    ALL.iter()
        .copied()
        .find(|def| def.stream == stream && def.id == id)
}
