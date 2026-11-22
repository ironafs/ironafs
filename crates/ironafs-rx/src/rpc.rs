// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The `Rpc` interface (ADR 0004, C3, point 3; seams and scheduling
//! specification, 5.4) and the values it carries.

use alloc::boxed::Box;
use alloc::vec::Vec;
use core::error::Error;
use core::fmt;
use core::net::SocketAddr;

use ironafs_time::{Deadline, Stamp, WatchRef};

#[cfg(test)]
mod tests;

/// Rx calls as the core makes and serves them (5.4, point 1).
///
/// `call` and `accept` block only on their own reply slot, through
/// `Clock::wait`, so under the simulator each is a yield point; `open`
/// and `respond` never block; `close` waits at most for one hold of its
/// connection's send lock already in progress (5.4, point 5). The cancel
/// token is a [`WatchRef`], the shared handle a pure crate can name; the
/// caller passes its cancel token's handle.
pub trait Rpc: Send + Sync {
    /// Opens an Rx connection. Sends nothing and blocks on nothing.
    ///
    /// # Errors
    ///
    /// An [`RpcError::NotSent`] cause if the connection cannot be opened.
    fn open(
        &self,
        peer: Peer,
        service: ServiceId,
        security: Box<dyn SecurityObject>,
    ) -> Result<ConnId, RpcError>;

    /// One client call: the encoded request out; reply bytes and evidence
    /// back. The reply is at most `reply_cap` bytes.
    ///
    /// # Errors
    ///
    /// `NotSent` only if the call owner ruled out transmission; `NoReply`
    /// if it may have been sent and no conclusive outcome exists; `Aborted`
    /// if the peer ended the call with an abort that matched it (5.4,
    /// point 2).
    fn call(
        &self,
        conn: ConnId,
        request: &[u8],
        reply_cap: u64,
        deadline: Deadline,
        cancel: &WatchRef,
    ) -> Result<Reply, RpcError>;

    /// Fences the connection before it returns, then settles every call of
    /// it with cause `Closed`: no datagram of the connection is accepted by
    /// the transport after `close` returns.
    fn close(&self, conn: ConnId);

    /// Server side, for the callback service (C31): the next incoming call
    /// for `service`.
    ///
    /// # Errors
    ///
    /// An [`RpcError`] whose cause is the deadline, the cancel token, a
    /// clock failure or a close.
    fn accept(
        &self,
        service: ServiceId,
        deadline: Deadline,
        cancel: &WatchRef,
    ) -> Result<Incoming, RpcError>;

    /// Answers an incoming call with reply bytes or an abort code.
    ///
    /// # Errors
    ///
    /// An [`RpcError`] if the answer cannot be queued.
    fn respond(&self, call: Incoming, reply: Response<'_>) -> Result<(), RpcError>;
}

/// The security-class interface of Rx: a pure state machine (C3, point 1).
/// Any credential work that needs I/O is done before [`Rpc::open`], by the
/// driver that obtains the token. Its operations are the Rx
/// specification's; until then it is the name `Rpc::open` takes.
pub trait SecurityObject: Send {}

/// The peer of a connection: its address and port.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Peer(pub SocketAddr);

/// An Rx service id.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ServiceId(pub u16);

/// A connection's handle, which the `Rpc` implementation assigns.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ConnId(pub u64);

/// The address family a reply arrived on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AddrFamily {
    /// IPv4.
    V4,
    /// IPv6.
    V6,
}

/// The generation of the local endpoint (its socket) a reply arrived on;
/// it changes when the endpoint is opened again, after a network change.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EndpointGeneration(pub u64);

/// A complete reply to a call (5.4, point 1).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reply {
    /// The reply's bytes, at most the call's cap.
    pub bytes: Vec<u8>,
    /// When the request was first handed to the transport (C26, point 2),
    /// a `Clock::sample` taken inside the hand-off. `None` if the sample
    /// failed: no lifetime is counted from it (C37, point 5).
    pub first_sent: Option<Stamp>,
    /// Where, on what and when the reply arrived.
    pub via: ReplyEvidence,
}

/// Where, on what and when a reply arrived (C32, point 1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReplyEvidence {
    /// The address family.
    pub family: AddrFamily,
    /// The local endpoint's generation.
    pub local_generation: EndpointGeneration,
    /// The connection's epoch.
    pub epoch: u32,
    /// The connection id on the wire.
    pub conn_id: u32,
    /// The call number.
    pub call_number: u32,
    /// A `Clock::sample` the loop took when it read the datagram that
    /// completed the reply, before feeding it to the endpoint. `None` if
    /// the sample failed.
    pub received: Option<Stamp>,
}

/// An incoming call for a service the core serves (C31): its peer, the
/// call identity the callback service checks, and the request bytes under
/// the service's cap.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Incoming {
    /// Who called.
    pub peer: Peer,
    /// The call's identity and when its request was read.
    pub identity: ReplyEvidence,
    /// The request's bytes.
    pub bytes: Vec<u8>,
}

/// The answer to an incoming call.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Response<'a> {
    /// Reply bytes.
    Reply(&'a [u8]),
    /// An abort code.
    Abort(u32),
}

/// What the call owner knows about transmission: the seam's only error
/// (5.4, point 2). The seam does not know whether an opcode changes state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RpcError {
    /// The call owner ruled out transmission. Safe to retry.
    NotSent(NotSentCause),
    /// Transmission may have occurred and no conclusive outcome exists.
    NoReply(NoReplyCause),
    /// The peer ended the call with an abort that matched the live call.
    /// The call is over; whether the procedure took effect is not
    /// established by the abort.
    Aborted {
        /// The abort code, classified by the RPC and the server that
        /// returned it, never by its number alone (C46, point 3).
        code: u32,
        /// Where and when the abort arrived.
        via: ReplyEvidence,
    },
}

/// Why a call was settled with nothing sent.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NotSentCause {
    /// The deadline passed, or was within the hand-off margin.
    Deadline,
    /// The caller's cancel token fired.
    Cancelled,
    /// The transport refused the first packet with an error a retry to the
    /// same destination cannot clear.
    Unreachable,
    /// A lane or the in-flight budget refused the call.
    Admission,
    /// The connection or the daemon was closed.
    Closed,
    /// The deadline's clock could not be read, or its doubt is latched.
    ClockFailed,
    /// The security object refused to protect the request.
    Security(SecurityFailure),
}

/// Why a call that may have been sent has no conclusive outcome.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NoReplyCause {
    /// The deadline passed, or the first packet was handed late.
    Deadline,
    /// The caller's cancel token fired.
    Cancelled,
    /// The peer stopped answering.
    PeerLost,
    /// The network changed under the call.
    NetworkChanged,
    /// The connection or the daemon was closed.
    Closed,
    /// The deadline's clock could not be read, or its doubt is latched.
    ClockFailed,
    /// The reply was over the call's cap: the peer ran the call.
    ReplyTooLarge,
    /// The reply failed the security object's check.
    Security(SecurityFailure),
}

/// A security object refused a request or a reply. Its detail is the Rx
/// specification's.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct SecurityFailure {
    // Private, so detail can be added without breaking callers.
    _private: (),
}

impl SecurityFailure {
    /// The error for a security object that refused.
    #[must_use]
    pub const fn new() -> Self {
        Self { _private: () }
    }
}

impl fmt::Display for RpcError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotSent(cause) => write!(formatter, "not sent: {}", not_sent(*cause)),
            Self::NoReply(cause) => write!(formatter, "no reply: {}", no_reply(*cause)),
            Self::Aborted { code, .. } => write!(formatter, "aborted with code {code}"),
        }
    }
}

impl Error for RpcError {}

const fn not_sent(cause: NotSentCause) -> &'static str {
    match cause {
        NotSentCause::Deadline => "deadline",
        NotSentCause::Cancelled => "cancelled",
        NotSentCause::Unreachable => "unreachable",
        NotSentCause::Admission => "admission",
        NotSentCause::Closed => "closed",
        NotSentCause::ClockFailed => "clock failed",
        NotSentCause::Security(_) => "security",
    }
}

const fn no_reply(cause: NoReplyCause) -> &'static str {
    match cause {
        NoReplyCause::Deadline => "deadline",
        NoReplyCause::Cancelled => "cancelled",
        NoReplyCause::PeerLost => "peer lost",
        NoReplyCause::NetworkChanged => "network changed",
        NoReplyCause::Closed => "closed",
        NoReplyCause::ClockFailed => "clock failed",
        NoReplyCause::ReplyTooLarge => "reply too large",
        NoReplyCause::Security(_) => "security",
    }
}
