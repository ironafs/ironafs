// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The consumers of the network generation and what each is told of a
//! move (docs/specs/network-generation.md, 5.3).
//!
//! The set is closed: [`Consumers`] has one field per consumer and no
//! default, so a source cannot be made with one of them missing.

use alloc::boxed::Box;
use core::fmt;

use crate::NetworkGeneration;

#[cfg(test)]
pub(crate) mod probe;

/// One consumer's hook, called on the monitor's thread for every move,
/// before the new generation is published. It must not block on the
/// network or on a lock the source's readers hold.
pub trait Consumer: Send {
    /// The generation moved, or the source stopped.
    fn moved(&mut self, change: Move);
}

/// What a consumer is told.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Move {
    /// The generation is now this one, for this cause. Everything kept
    /// under an earlier generation is stale.
    To(NetworkGeneration, Cause),
    /// The source stopped: no generation is current from now on.
    Stopped,
}

/// Why the generation moved.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cause {
    /// A link, address, route, policy rule or nexthop changed.
    Changed,
    /// A snapshot completed: a change between the subscription and its
    /// end may be in it without having moved the generation.
    Snapshot,
    /// The monitor cannot know it saw every change.
    Lost(Loss),
}

/// What made the monitor lose track (docs/specs/network-generation.md,
/// section 7).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Loss {
    /// The kernel dropped notifications for lack of room.
    Overflow,
    /// A datagram was larger than the buffer.
    Truncated,
    /// A datagram failed decoding, or the kernel reported an overrun.
    Malformed,
    /// A dump was inconsistent (`NLM_F_DUMP_INTR`).
    Interrupted,
    /// The kernel answered a dump with an error.
    Refused,
    /// A socket failed.
    Failed,
    /// The machine resumed from suspend.
    Resumed,
}

/// Every consumer's hook, in the order they are called.
pub struct Consumers {
    /// The negative cache is cleared (ADR 0004, C12, point 3).
    pub negative_cache: Box<dyn Consumer>,
    /// Grants carry the new generation; outstanding ones are fenced (C23,
    /// point 2).
    pub grants: Box<dyn Consumer>,
    /// Per-server trust is revalidated (C32, point 1).
    pub server_trust: Box<dyn Consumer>,
    /// Cached DNS records are discarded (C33, point 6).
    pub dns_records: Box<dyn Consumer>,
    /// Location records and server rankings are dropped (C33, point 6).
    pub dns_rankings: Box<dyn Consumer>,
    /// Replies to requests sent before the move are refused (C33, point 6).
    pub dns_late_replies: Box<dyn Consumer>,
    /// The flat-list confirmation is checked afresh (C33, point 4).
    pub dns_flat_list: Box<dyn Consumer>,
}

impl Consumers {
    /// Tells every consumer, in the order of the fields.
    pub(crate) fn tell(&mut self, change: Move) {
        for consumer in [
            &mut self.negative_cache,
            &mut self.grants,
            &mut self.server_trust,
            &mut self.dns_records,
            &mut self.dns_rankings,
            &mut self.dns_late_replies,
            &mut self.dns_flat_list,
        ] {
            consumer.moved(change);
        }
    }
}

impl fmt::Debug for Consumers {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Consumers { .. }")
    }
}
