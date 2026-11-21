// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! IronAFS's network generation (docs/specs/network-generation.md; ADR
//! 0004, C23, point 2): a counter that moves whenever the host's links,
//! addresses, routes, policy rules or nexthops change, or whenever the
//! monitor cannot be sure it saw every change; the hooks of the consumers
//! that must drop what they kept under an older generation; and the Linux
//! monitor's route-netlink decoder and state machine.
//!
//! This crate does no I/O. A driver owns the sockets, hands each datagram
//! to [`Monitor`] and sends the dump requests it asks for.

#![no_std]

extern crate alloc;

mod consumer;
mod generation;

pub use consumer::{Cause, Consumer, Consumers, Loss, Move};
pub use generation::{NetworkGeneration, Reader, Unavailable};
