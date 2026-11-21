// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! IronAFS's deterministic simulator (ADR 0004, O45; C3, point 6): a
//! test-only crate, never shipped (C4, point 7).

// A crash marker unwinds a task's stack, and a panic in a task must reach
// the scheduler as a failure, not end the process (seams and scheduling
// specification, 5.6, point 8).
#[cfg(panic = "abort")]
compile_error!("ironafs-sim needs unwinding panics: build it without panic = \"abort\"");

pub mod child;
pub mod replay;
pub mod scenario;
pub mod sched;
pub mod stream;
pub mod supervisor;
pub mod trace;
pub mod witness;
