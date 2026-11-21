// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Building an event: the producer-role check, the direct-write flag, and
//! field values checked against their definition (`docs/specs/events.md`
//! §5.5; ADR 0004, O3, point 2, O4, points 2 and 5, and O9, point 2).

use core::error::Error;
use core::fmt;

use crate::catalogue::{self, EventDef, MAX_FIELDS, Role, Stream};
use crate::field::{FieldSource, Value};

#[cfg(test)]
mod tests;

/// Why an event could not be built (`docs/specs/events.md` §7, point 1).
/// No variant carries a field's value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventError {
    /// The definition does not list this producer role.
    RoleNotAllowed {
        /// The event's name.
        event: &'static str,
        /// The role that tried to emit it.
        role: Role,
    },
    /// A direct write of an audit event (ADR 0004, O4, point 5).
    DirectAudit {
        /// The event's name.
        event: &'static str,
    },
    /// The definition has no field of this name.
    UnknownField {
        /// The event's name.
        event: &'static str,
    },
    /// The value's kind is not the field's kind.
    KindMismatch {
        /// The event's name.
        event: &'static str,
        /// The field's name.
        field: &'static str,
    },
    /// The field already has a value.
    DuplicateField {
        /// The event's name.
        event: &'static str,
        /// The field's name.
        field: &'static str,
    },
    /// A static text value that is not one of the field's catalogue
    /// choices.
    ValueNotAllowed {
        /// The event's name.
        event: &'static str,
        /// The field's name.
        field: &'static str,
    },
    /// A required field has no value.
    MissingField {
        /// The event's name.
        event: &'static str,
        /// The field's name.
        field: &'static str,
    },
}

impl fmt::Display for EventError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RoleNotAllowed { event, role } => {
                write!(formatter, "role {role:?} may not emit event {event}")
            }
            Self::DirectAudit { event } => {
                write!(formatter, "audit event {event} cannot be a direct write")
            }
            Self::UnknownField { event } => {
                write!(formatter, "event {event} has no field of that name")
            }
            Self::KindMismatch { event, field } => {
                write!(formatter, "field {field} of event {event} has another kind")
            }
            Self::DuplicateField { event, field } => {
                write!(formatter, "field {field} of event {event} is already set")
            }
            Self::ValueNotAllowed { event, field } => {
                write!(
                    formatter,
                    "field {field} of event {event} has no such choice"
                )
            }
            Self::MissingField { event, field } => {
                write!(
                    formatter,
                    "required field {field} of event {event} is missing"
                )
            }
        }
    }
}

impl Error for EventError {}

/// An event under construction: a definition, the role emitting it, and
/// field values checked one by one. Its `Debug` shows no value.
#[derive(Clone)]
pub struct EventBuilder {
    event: Event,
}

impl fmt::Debug for EventBuilder {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("EventBuilder")
            .field(&self.event)
            .finish()
    }
}

/// A complete event: every required field has a value of its field's kind.
///
/// Its `Debug` shows the event's name, the direct flag and the names of the
/// fields set, never a value: redaction applies only when an encoder writes
/// the event (`docs/specs/events.md` §5.4).
#[derive(Clone, PartialEq, Eq)]
pub struct Event {
    def: &'static EventDef,
    direct: bool,
    values: [Option<Value>; MAX_FIELDS],
}

impl fmt::Debug for Event {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let set = self
            .def
            .fields
            .iter()
            .enumerate()
            .filter(|(index, _)| self.value(*index).is_some())
            .map(|(_, spec)| spec.name);
        formatter
            .debug_struct("Event")
            .field("event", &self.def.name)
            .field("direct", &self.direct)
            .field("fields", &FieldNames(set))
            .finish_non_exhaustive()
    }
}

/// Formats field names as a list, from an iterator it can clone.
struct FieldNames<I>(I);

impl<I: Iterator<Item = &'static str> + Clone> fmt::Debug for FieldNames<I> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_list().entries(self.0.clone()).finish()
    }
}

impl Event {
    /// Starts an event of `def`, emitted by `role`.
    ///
    /// ```
    /// use ironafs_events::{Event, Role, ServerText, catalogue};
    ///
    /// let event = Event::builder(&catalogue::VOLUME_OFFLINE, Role::Daemon)?
    ///     .with("volume", 536_870_915_u64)?
    ///     .with("message", ServerText::from_wire(b"salvaging"))?
    ///     .build()?;
    /// assert_eq!(event.def().name(), "volume.offline");
    /// # Ok::<(), ironafs_events::EventError>(())
    /// ```
    ///
    /// # Errors
    ///
    /// [`EventError::RoleNotAllowed`] if the definition does not list
    /// `role`.
    pub fn builder(def: &'static EventDef, role: Role) -> Result<EventBuilder, EventError> {
        if !def.producers.contains(role) {
            return Err(EventError::RoleNotAllowed {
                event: def.name,
                role,
            });
        }
        Ok(EventBuilder {
            event: Self {
                def,
                direct: false,
                values: Default::default(),
            },
        })
    }

    /// The "event dropped: over the size limit" event that replaces an
    /// event an encoder refused with
    /// [`EncodeError::OverSizeLimit`](crate::EncodeError::OverSizeLimit)
    /// (ADR 0005, point 1.7).
    ///
    /// # Errors
    ///
    /// None for any role: every role may emit it. The result type is the
    /// builder's.
    pub fn over_size(stream: Stream, id: u16, role: Role) -> Result<Self, EventError> {
        Self::builder(&catalogue::EVENTS_OVER_SIZE, role)?
            .with("stream", stream.name())?
            .with("event", id)?
            .build()
    }

    /// The event's definition.
    #[must_use]
    pub fn def(&self) -> &'static EventDef {
        self.def
    }

    /// Whether the event is a direct write (ADR 0004, O4, point 2).
    #[must_use]
    pub fn is_direct(&self) -> bool {
        self.direct
    }

    /// The value of the field at `index` in the definition, if set.
    pub(crate) fn value(&self, index: usize) -> Option<&Value> {
        self.values.get(index).and_then(Option::as_ref)
    }
}

impl EventBuilder {
    /// Marks the event as a direct write (ADR 0004, O4, point 2).
    ///
    /// # Errors
    ///
    /// [`EventError::DirectAudit`] for an audit event: a direct write is
    /// never audit (O4, point 5).
    pub fn direct(mut self) -> Result<Self, EventError> {
        if self.event.def.stream == Stream::Audit {
            return Err(EventError::DirectAudit {
                event: self.event.def.name,
            });
        }
        self.event.direct = true;
        Ok(self)
    }

    /// Sets the field called `name`.
    ///
    /// # Errors
    ///
    /// [`EventError::UnknownField`], [`EventError::KindMismatch`],
    /// [`EventError::ValueNotAllowed`] or [`EventError::DuplicateField`].
    pub fn with(mut self, name: &str, value: impl FieldSource) -> Result<Self, EventError> {
        let def = self.event.def;
        let (spec, slot) = def
            .field_index(name)
            .and_then(|index| Some((def.fields.get(index)?, self.event.values.get_mut(index)?)))
            .ok_or(EventError::UnknownField { event: def.name })?;
        let mut value = value.into_value();
        if value.kind() != spec.kind {
            return Err(EventError::KindMismatch {
                event: def.name,
                field: spec.name,
            });
        }
        if let Value::Static(text) = value {
            // Keep the catalogue's own string: static text is only ever one
            // of the field's choices, whatever the caller's string was.
            let choice = spec.choices.iter().find(|choice| **choice == text);
            value = Value::Static(choice.ok_or(EventError::ValueNotAllowed {
                event: def.name,
                field: spec.name,
            })?);
        }
        if slot.is_some() {
            return Err(EventError::DuplicateField {
                event: def.name,
                field: spec.name,
            });
        }
        *slot = Some(value);
        Ok(self)
    }

    /// Finishes the event.
    ///
    /// # Errors
    ///
    /// [`EventError::MissingField`] if a required field has no value.
    pub fn build(self) -> Result<Event, EventError> {
        let def = self.event.def;
        for (index, spec) in def.fields.iter().enumerate() {
            if spec.required && self.event.value(index).is_none() {
                return Err(EventError::MissingField {
                    event: def.name,
                    field: spec.name,
                });
            }
        }
        Ok(self.event)
    }
}
