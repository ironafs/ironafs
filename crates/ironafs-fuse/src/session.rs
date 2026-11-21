// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! One FUSE connection's codec state (fuse-codec.md 5.6, point 3): its
//! phase, and the requests still owed a reply.
//!
//! A [`Session`] is the only way to decode a request and the only way to
//! turn a [`Reply`] into bytes for the device. It issues a request's reply
//! token once for each pending request id, and retires the id when that
//! reply is finished, so a reply cannot be duplicated by decoding a frame
//! twice, sent on another connection, or sent for a request nobody is
//! owed.

use core::sync::atomic::{AtomicU64, Ordering};
use std::collections::BTreeSet;

use crate::Opcode;
use crate::reply::Reply;
use crate::request::{self, DecodeError, Header, Phase, Request, RequestError};

#[cfg(test)]
mod tests;

/// The next session's id. Each session takes a distinct one, so a reply
/// can name the session that issued its token.
static NEXT_SESSION: AtomicU64 = AtomicU64::new(1);

/// A connection's codec state.
#[derive(Debug)]
pub struct Session {
    id: u64,
    phase: Phase,
    /// The request ids whose reply token is outstanding.
    pending: BTreeSet<u64>,
}

/// Why [`Session::finish`] refused a reply.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FinishError {
    /// The reply's token came from another session.
    OtherSession,
    /// No request with the reply's id is owed one.
    NotPending,
}

impl core::fmt::Display for FinishError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::OtherSession => "the reply belongs to another fuse session",
            Self::NotPending => "no request with the reply's id is owed one",
        })
    }
}

impl core::error::Error for FinishError {}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

impl Session {
    /// A new connection's state: awaiting INIT, nothing pending.
    #[must_use]
    pub fn new() -> Self {
        Self {
            id: NEXT_SESSION.fetch_add(1, Ordering::Relaxed),
            phase: Phase::AwaitingInit,
            pending: BTreeSet::new(),
        }
    }

    /// The negotiated minor, once an INIT reply has been finished.
    #[must_use]
    pub const fn minor(&self) -> Option<u32> {
        match self.phase {
            Phase::AwaitingInit => None,
            Phase::Negotiated { minor } => Some(minor),
        }
    }

    /// How many requests are owed a reply.
    #[must_use]
    pub fn pending(&self) -> usize {
        self.pending.len()
    }

    /// Decodes one request read from the device (fuse-codec.md 5.4).
    ///
    /// # Errors
    ///
    /// As [`request`]'s decoding says.
    pub fn decode<'a>(&mut self, bytes: &'a [u8]) -> Result<(Header, Request<'a>), DecodeError> {
        // A request id still owed a reply gets no second token: a reply
        // under it would answer the earlier request (5.6, point 3).
        if let Some(header) = request::peek_header(bytes)
            && takes_reply(&header)
            && self.pending.contains(&header.unique)
        {
            return Err(DecodeError::Request(RequestError::duplicate(header)));
        }
        let result = request::decode(bytes, self.phase, self.id);
        let issued = match &result {
            Ok((header, _)) => Some(header),
            Err(DecodeError::Request(error)) if error.errno().is_some() => Some(&error.header),
            Err(_) => None,
        };
        if let Some(header) = issued
            && takes_reply(header)
        {
            self.pending.insert(header.unique);
        }
        result
    }

    /// The bytes to write for `reply`, retiring its request id. A finished
    /// INIT reply that settled the protocol moves the session to its minor.
    ///
    /// # Errors
    ///
    /// [`FinishError`] for a reply of another session, or one no request
    /// is owed.
    pub fn finish(&mut self, reply: Reply) -> Result<Vec<u8>, FinishError> {
        if reply.session() != self.id {
            return Err(FinishError::OtherSession);
        }
        if !self.pending.remove(&reply.unique()) {
            return Err(FinishError::NotPending);
        }
        if let Some(negotiated) = reply.negotiated() {
            self.phase = Phase::Negotiated {
                minor: negotiated.minor(),
            };
        }
        Ok(reply.into_bytes())
    }
}

/// Whether a request with this header is given a reply token: every
/// request except FORGET, `BATCH_FORGET` and INTERRUPT, unknown opcodes
/// included.
fn takes_reply(header: &Header) -> bool {
    Opcode::from_u32(header.opcode).is_none_or(Opcode::needs_reply)
}
