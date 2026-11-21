// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The calls a fileserver model sends to a client's callback service, and
//! their answers, in the simulator's own envelope.
//!
//! The procedures are the allowlisted ones of `docs/specs/callbacks.md`
//! (5.1.2) that a server sends: a break, the three resets and the probe,
//! by their IronAFS names. That specification marks every argument and
//! result layout MISSING (5.1.3, 5.5), so this is not the AFS-3 wire:
//! like [`super::envelope`], a tag byte per variant, big-endian integers
//! and counted lists, decoded strictly. The source address travels in
//! the envelope, as the address a datagram comes from would, so a forged
//! call is one whose source no server chose.

use super::envelope::{EnvelopeError, Reader, Writer};
use super::types::{Addr, ClientId, Fid};

#[cfg(test)]
mod tests;

/// Which of the three reset procedures (5.1.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reset {
    /// `cb.init_state`.
    One,
    /// `cb.init_state2`.
    Two,
    /// `cb.init_state3`.
    Three,
}

/// A procedure a server calls on the callback service.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CbProc {
    /// `cb.callback`: a break naming these files.
    CallBack {
        /// The files broken.
        fids: Vec<Fid>,
    },
    /// A reset of the client's callback state.
    InitState(Reset),
    /// `cb.probe`.
    Probe,
}

impl CbProc {
    /// The IronAFS name (5.1.2).
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::CallBack { .. } => "cb.callback",
            Self::InitState(Reset::One) => "cb.init_state",
            Self::InitState(Reset::Two) => "cb.init_state2",
            Self::InitState(Reset::Three) => "cb.init_state3",
            Self::Probe => "cb.probe",
        }
    }
}

/// One call to a client's callback service.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CbRequest {
    /// The source address the call carries.
    pub from: Addr,
    /// The client host called.
    pub to: ClientId,
    /// The procedure.
    pub procedure: CbProc,
}

/// The callback service's answer: an empty result, or an abort.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CbAnswer {
    /// An empty result (5.5, point 1).
    Empty,
    /// An abort with this code (5.7, point 1).
    Abort(i32),
}

/// The envelope bytes of `request`.
#[must_use]
pub fn encode_cb_request(request: &CbRequest) -> Vec<u8> {
    let mut out = Writer::default();
    out.addr(request.from);
    out.u32(request.to.0);
    match &request.procedure {
        CbProc::CallBack { fids } => {
            out.u8(0);
            out.fids(fids);
        }
        CbProc::InitState(reset) => {
            out.u8(1);
            out.u8(match reset {
                Reset::One => 0,
                Reset::Two => 1,
                Reset::Three => 2,
            });
        }
        CbProc::Probe => out.u8(2),
    }
    out.into_bytes()
}

/// The request `bytes` encode.
///
/// # Errors
///
/// An [`EnvelopeError`] if `bytes` are not exactly one request.
pub fn decode_cb_request(bytes: &[u8]) -> Result<CbRequest, EnvelopeError> {
    let mut input = Reader::new(bytes);
    let from = input.addr()?;
    let to = ClientId(input.u32()?);
    let procedure = match input.u8()? {
        0 => CbProc::CallBack {
            fids: input.fids()?,
        },
        1 => CbProc::InitState(match input.u8()? {
            0 => Reset::One,
            1 => Reset::Two,
            2 => Reset::Three,
            _ => return Err(EnvelopeError::Tag),
        }),
        2 => CbProc::Probe,
        _ => return Err(EnvelopeError::Tag),
    };
    input.end()?;
    Ok(CbRequest {
        from,
        to,
        procedure,
    })
}

/// The envelope bytes of `answer`.
#[must_use]
pub fn encode_cb_answer(answer: &CbAnswer) -> Vec<u8> {
    let mut out = Writer::default();
    match answer {
        CbAnswer::Abort(code) => {
            out.u8(0);
            out.i32(*code);
        }
        CbAnswer::Empty => out.u8(1),
    }
    out.into_bytes()
}

/// The answer `bytes` encode.
///
/// # Errors
///
/// An [`EnvelopeError`] if `bytes` are not exactly one answer.
pub fn decode_cb_answer(bytes: &[u8]) -> Result<CbAnswer, EnvelopeError> {
    let mut input = Reader::new(bytes);
    let answer = match input.u8()? {
        0 => CbAnswer::Abort(input.i32()?),
        1 => CbAnswer::Empty,
        _ => return Err(EnvelopeError::Tag),
    };
    input.end()?;
    Ok(answer)
}
