// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The simulator's volume location and fileserver models (ADR 0004, O45,
//! point 1), written from the specification of volume location and
//! file-service reads (`docs/specs/vl-fs-read.md`) and from nothing else
//! (ADR 0001).
//!
//! The models work at the level of procedures and results: that
//! specification marks every byte layout MISSING (its 5.1), so no model
//! encodes or decodes the AFS-3 wire. A request and its answer cross the
//! scheduler's server seam in the simulator's own [`envelope`], which no
//! client crate reads and which shares no code with any client codec.

pub mod callback;
pub mod cell;
pub mod check;
pub mod codes;
pub mod envelope;
pub mod front;
pub mod fs;
pub mod types;
pub mod vl;
