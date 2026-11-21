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
    pub(crate) name: &'static str,
    pub(crate) kind: FieldKind,
    pub(crate) class: DataClass,
    pub(crate) required: bool,
    pub(crate) choices: &'static [&'static str],
}

impl FieldSpec {
    /// The field's name: lower-case ASCII letters, digits and `_`, at most
    /// 24 bytes.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        self.name
    }

    /// The value's type.
    #[must_use]
    pub const fn kind(&self) -> FieldKind {
        self.kind
    }

    /// What the value may reveal.
    #[must_use]
    pub const fn class(&self) -> DataClass {
        self.class
    }

    /// Whether every event of this definition carries it.
    #[must_use]
    pub const fn required(&self) -> bool {
        self.required
    }

    /// For a static text field, the only values it takes; empty for every
    /// other kind (`docs/specs/events.md` §5.2, point 3).
    #[must_use]
    pub const fn choices(&self) -> &'static [&'static str] {
        self.choices
    }
}

/// One catalogue entry (ADR 0004, O1, point 2).
///
/// Only the catalogue defines events: no other crate can build an
/// `EventDef`, so every definition an event, the limiter or an encoder
/// sees is one the catalogue's tests check.
///
/// ```compile_fail,E0451
/// use ironafs_events::{Audience, EventDef, Roles, Severity, Stream, Subsystem};
///
/// static FORGED: EventDef = EventDef {
///     stream: Stream::Audit,
///     id: 9,
///     version: 1,
///     name: "forged",
///     severity: Severity::Notice,
///     audience: Audience::Audit,
///     subsystem: Subsystem::Daemon,
///     fields: &[],
///     template: "anything",
///     meaning: "",
///     remedy: "",
///     producers: Roles::ALL,
/// };
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EventDef {
    pub(crate) stream: Stream,
    pub(crate) id: u16,
    pub(crate) version: u8,
    pub(crate) name: &'static str,
    pub(crate) severity: Severity,
    pub(crate) audience: Audience,
    pub(crate) subsystem: Subsystem,
    pub(crate) fields: &'static [FieldSpec],
    pub(crate) template: &'static str,
    pub(crate) meaning: &'static str,
    pub(crate) remedy: &'static str,
    pub(crate) producers: Roles,
}

impl EventDef {
    /// The stream; ids are unique within it.
    #[must_use]
    pub const fn stream(&self) -> Stream {
        self.stream
    }

    /// The id, never reused (ADR 0004, O2, point 1).
    #[must_use]
    pub const fn id(&self) -> u16 {
        self.id
    }

    /// The version, from 1.
    #[must_use]
    pub const fn version(&self) -> u8 {
        self.version
    }

    /// The name: lower-case ASCII letters, digits, `.` and `_`, at most 32
    /// bytes, unique in the catalogue.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        self.name
    }

    /// The severity.
    #[must_use]
    pub const fn severity(&self) -> Severity {
        self.severity
    }

    /// Who the event is for.
    #[must_use]
    pub const fn audience(&self) -> Audience {
        self.audience
    }

    /// Where it comes from.
    #[must_use]
    pub const fn subsystem(&self) -> Subsystem {
        self.subsystem
    }

    /// Its fields, at most [`MAX_FIELDS`].
    #[must_use]
    pub const fn fields(&self) -> &'static [FieldSpec] {
        self.fields
    }

    /// The message, with `{name}` placeholders for fields.
    #[must_use]
    pub const fn template(&self) -> &'static str {
        self.template
    }

    /// What the event means.
    #[must_use]
    pub const fn meaning(&self) -> &'static str {
        self.meaning
    }

    /// What an operator does about it.
    #[must_use]
    pub const fn remedy(&self) -> &'static str {
        self.remedy
    }

    /// The roles allowed to emit it.
    #[must_use]
    pub const fn producers(&self) -> Roles {
        self.producers
    }

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
