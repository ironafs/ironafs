// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! IronAFS's clock and randomness interfaces and its time types (ADR 0004,
//! C3, point 3; C4, points 1 and 7; C37; C38).
//!
//! Every read of time and every random draw in the client goes through the
//! [`Clock`] and [`Rng`] interfaces here (coding standards rule 7), so the
//! simulator can drive the client from one seed and timers can be tested
//! without waiting. This crate holds the interfaces and value types only:
//! the production clocks live in the operating-system bindings, the
//! production random source behind the cryptography interface, and the
//! seeded implementations in the test support crate.
//!
//! There are three clocks (ADR 0004, C37, point 1):
//!
//! - the **boot** clock counts time the machine was suspended. Every
//!   deadline a server grants (callbacks, tokens, location lifetimes,
//!   locks) counts on it: [`BootInstant`], [`BootDeadline`];
//! - the **awake** clock excludes suspend. Round-trip and retransmission
//!   timers count on it, so a timer does not fire a burst on waking:
//!   [`AwakeInstant`], [`AwakeDeadline`];
//! - **wall** time is only for AFS timestamps and for token expiry as the
//!   protocol states it: [`WallTime`]. No deadline counts on it.
//!
//! All time inside IronAFS is 64-bit (ADR 0004, C38, point 1): boot and
//! awake time are unsigned 64-bit nanoseconds, and wall time is signed
//! 64-bit seconds since 1970 with a nanosecond part. Arithmetic is checked
//! and never wraps.
//!
//! An instant of one clock cannot be compared with, or turned into a
//! deadline on, another clock: each clock has its own types.
//!
//! ```compile_fail,E0308
//! use ironafs_time::{AwakeInstant, BootDeadline, BootInstant};
//!
//! let deadline = BootDeadline::at(BootInstant::from_nanos(10));
//! // A boot-clock deadline cannot be checked against the awake clock.
//! let _ = deadline.has_passed(AwakeInstant::from_nanos(20));
//! ```

#![no_std]

extern crate alloc;

mod instant;
mod seam;
mod wait;
mod wall;

pub use instant::{
    Awake, AwakeDeadline, AwakeInstant, Boot, BootDeadline, BootInstant, MonotonicClock,
    MonotonicDeadline, MonotonicInstant,
};
pub use seam::{Clock, ClockError, ClockKind, Rng, RngError};
pub use wait::{
    Deadline, DeadlineError, Doubt, Registration, Stamp, Waited, Waker, Watch, WatchFull, WatchRef,
};
pub use wall::WallTime;
