// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The catalogue's entries (`docs/specs/events.md` §5.1, point 8). No
//! entry is published yet, so none is frozen (ADR 0004, O2, point 2).

use super::{
    Audience, DataClass, EventDef, FieldKind, FieldSpec, Roles, Severity, Stream, Subsystem,
};

const fn field(name: &'static str, kind: FieldKind, class: DataClass, required: bool) -> FieldSpec {
    FieldSpec {
        name,
        kind,
        class,
        required,
        choices: &[],
    }
}

/// A required static text field that takes only `choices`.
const fn choice(name: &'static str, choices: &'static [&'static str]) -> FieldSpec {
    FieldSpec {
        name,
        kind: FieldKind::Static,
        class: DataClass::Public,
        required: true,
        choices,
    }
}

/// The reasons `events.lost` names: [`Loss::reason`](crate::Loss::reason)
/// of each loss.
const LOSS_REASONS: &[&str] = &[
    "over the size limit",
    "queue full",
    "sink stalled",
    "sink unavailable",
    "truncated, delivered in part",
];

/// The subsystems' names, as [`Subsystem::name`] gives them.
const SUBSYSTEM_NAMES: &[&str] = &[
    "events", "daemon", "rx", "server", "volume", "cache", "config", "adapter",
];

/// The streams' names, as [`Stream::name`] gives them.
const STREAM_NAMES: &[&str] = &["diagnostic", "audit"];

/// The kinds of audit gap (ADR 0004, O9, point 4).
const GAP_KINDS: &[&str] = &["known-loss", "uncertain", "possible-loss"];

/// Events lost at a bound, with the reason and how many (ADR 0004, O3,
/// point 3, and O7, point 3).
pub static EVENTS_LOST: EventDef = EventDef {
    stream: Stream::Diagnostic,
    id: 1,
    version: 1,
    name: "events.lost",
    severity: Severity::Warning,
    audience: Audience::Operational,
    subsystem: Subsystem::Events,
    fields: &[
        choice("reason", LOSS_REASONS),
        field("count", FieldKind::Unsigned, DataClass::Public, true),
    ],
    template: "{count} events lost: {reason}",
    meaning: "Events were not delivered to the log because a bound was reached.",
    remedy: "If it repeats, look for the cause of the event volume or the stalled log sink.",
    producers: Roles::ALL,
};

/// Events a subsystem's rate limit refused, summarised (ADR 0004, O7,
/// points 1 and 2).
pub static EVENTS_SUPPRESSED: EventDef = EventDef {
    stream: Stream::Diagnostic,
    id: 2,
    version: 1,
    name: "events.suppressed",
    severity: Severity::Notice,
    audience: Audience::Operational,
    subsystem: Subsystem::Events,
    fields: &[
        choice("subsystem", SUBSYSTEM_NAMES),
        field("count", FieldKind::Unsigned, DataClass::Public, true),
    ],
    template: "{count} {subsystem} events suppressed by the rate limit",
    meaning: "A subsystem emitted more events than its limit allows; these were counted and not logged.",
    remedy: "Look at the subsystem's first events before the suppression for the cause.",
    producers: Roles::ALL,
};

/// "Event dropped: over the size limit" (ADR 0005, point 1.7). Its fields
/// name the dropped event.
pub static EVENTS_OVER_SIZE: EventDef = EventDef {
    stream: Stream::Diagnostic,
    id: 3,
    version: 1,
    name: "events.over_size",
    severity: Severity::Warning,
    audience: Audience::Operational,
    subsystem: Subsystem::Events,
    fields: &[
        choice("stream", STREAM_NAMES),
        field("event", FieldKind::Unsigned, DataClass::Public, true),
    ],
    template: "event dropped: over the size limit ({stream} event {event})",
    meaning: "An event's identity and required fields did not fit the log sink's size limit, so it was not delivered.",
    remedy: "Report it: the event's definition needs a smaller encoding.",
    producers: Roles::ALL,
};

/// A server stopped answering. Logged once per state change (ADR 0004, O7,
/// point 1).
pub static SERVER_UNREACHABLE: EventDef = EventDef {
    stream: Stream::Diagnostic,
    id: 100,
    version: 1,
    name: "server.unreachable",
    severity: Severity::Error,
    audience: Audience::Admin,
    subsystem: Subsystem::Server,
    fields: &[field(
        "server",
        FieldKind::Reference,
        DataClass::Reference,
        true,
    )],
    template: "server {server} unreachable",
    meaning: "A server stopped answering; requests to it fail over or wait.",
    remedy: "Check the server and the network path to it.",
    producers: Roles::DAEMON,
};

/// A server answers again (ADR 0004, O7, point 1).
pub static SERVER_REACHABLE: EventDef = EventDef {
    stream: Stream::Diagnostic,
    id: 101,
    version: 1,
    name: "server.reachable",
    severity: Severity::Notice,
    audience: Audience::Admin,
    subsystem: Subsystem::Server,
    fields: &[field(
        "server",
        FieldKind::Reference,
        DataClass::Reference,
        true,
    )],
    template: "server {server} reachable",
    meaning: "A server that was unreachable answers again.",
    remedy: "None.",
    producers: Roles::DAEMON,
};

/// A volume is offline, with the server's own message.
pub static VOLUME_OFFLINE: EventDef = EventDef {
    stream: Stream::Diagnostic,
    id: 200,
    version: 1,
    name: "volume.offline",
    severity: Severity::Warning,
    audience: Audience::Operational,
    subsystem: Subsystem::Volume,
    fields: &[
        field("volume", FieldKind::Unsigned, DataClass::Identifier, true),
        field(
            "message",
            FieldKind::ServerText,
            DataClass::ServerText,
            false,
        ),
    ],
    template: "volume {volume} offline: {message}",
    meaning: "A file server reports a volume offline; reads of it fail until it returns.",
    remedy: "Ask the cell's administrators; the server's message may say why.",
    producers: Roles::DAEMON,
};

/// Audit events lost or of uncertain outcome, by sequence range (ADR 0004,
/// O9, point 4). `kind` is `known-loss`, `uncertain` or `possible-loss`.
pub static AUDIT_GAP: EventDef = EventDef {
    stream: Stream::Audit,
    id: 1,
    version: 1,
    name: "audit.gap",
    severity: Severity::Warning,
    audience: Audience::Audit,
    subsystem: Subsystem::Daemon,
    fields: &[
        field("first", FieldKind::Unsigned, DataClass::Public, true),
        field("last", FieldKind::Unsigned, DataClass::Public, true),
        choice("kind", GAP_KINDS),
    ],
    template: "audit events {first} to {last}: {kind}",
    meaning: "Audit events in this sequence range were lost, or may have been.",
    remedy: "Treat the range as missing from the audit record, and look for the stalled or failed sink.",
    producers: Roles::DAEMON,
};

/// Every entry of the catalogue.
pub static ALL: &[&EventDef] = &[
    &EVENTS_LOST,
    &EVENTS_SUPPRESSED,
    &EVENTS_OVER_SIZE,
    &SERVER_UNREACHABLE,
    &SERVER_REACHABLE,
    &VOLUME_OFFLINE,
    &AUDIT_GAP,
];
