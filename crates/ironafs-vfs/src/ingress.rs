// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! What the ingress records for each request from the operating system
//! (ADR 0004, C39, points 2 and 3).
//!
//! A layer's reader threads only read requests, record their arrival and
//! hand them on (C39, point 2). A worker's wait has a deadline counted from
//! the arrival, and a cancel token (C39, point 3). The deadline itself is
//! built by the core, from a fresh sample and this record's `arrival`
//! (seams and scheduling specification, 5.3, points 1 and 2), so a layer
//! never hands the core a deadline. Which clock a request's deadline is on
//! is not settled here (that specification, section 13, Q3).

use core::fmt;

use ironafs_time::{Stamp, WatchRef};

/// A request's id on its binding, which completions and `cancel` name.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RequestId(pub u64);

/// A request as the ingress recorded it.
///
/// An ingress whose clock sample failed records no arrival and builds no
/// `Ingress`; no deadline is set without a valid sample (C37, point 5).
pub struct Ingress {
    /// The request's id.
    pub request: RequestId,
    /// The sample the reader took when the request arrived.
    pub arrival: Stamp,
    /// Set when the request is cancelled or interrupted.
    pub cancel: WatchRef,
}

impl fmt::Debug for Ingress {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Ingress")
            .field("request", &self.request)
            .field("arrival", &self.arrival)
            .field("cancelled", &self.cancel.is_set())
            .finish()
    }
}
