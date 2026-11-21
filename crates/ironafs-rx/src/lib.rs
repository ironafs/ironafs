// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! IronAFS's Rx (ADR 0004, C4, point 1), which knows nothing of AFS
//! (point 4).
//!
//! For now it holds the `Rpc` interface only (C3, point 3; seams and
//! scheduling specification, 5.4): [`Rpc`], its errors, and the values it
//! takes and returns. Its vocabulary is peers, services, connections,
//! security objects and bytes. It is a pure crate and never calls `Rpc`
//! itself (`cargo xtask check-seams`); the Rx state machine, the
//! security-class interface's methods and the null class come with the Rx
//! specification.

#![no_std]

extern crate alloc;

mod rpc;

pub use rpc::{
    AddrFamily, ConnId, EndpointGeneration, Incoming, NoReplyCause, NotSentCause, Peer, Reply,
    ReplyEvidence, Response, Rpc, RpcError, SecurityFailure, SecurityObject, ServiceId,
};
