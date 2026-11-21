// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for the rate limiter and the loss counters
//! (`docs/specs/events.md` §5.6).

use super::{Admission, LIMIT_MAX, LimitConfig, LimitError, Loss, LossCounters, RateLimiter};
use crate::catalogue::{self, Role, Subsystem};
use crate::field::Value;

const SECOND: u64 = 1_000_000_000;

fn limiter(rate: u32, burst: u32) -> RateLimiter {
    RateLimiter::new(LimitConfig::new(rate, burst).unwrap())
}

#[test]
fn a_burst_is_admitted_then_the_rest_suppressed() {
    let mut limiter = limiter(1, 3);
    let def = &catalogue::SERVER_UNREACHABLE;
    for _ in 0..3 {
        assert_eq!(limiter.admit(def, 5 * SECOND), Admission::Admitted);
    }
    assert_eq!(limiter.admit(def, 5 * SECOND), Admission::Suppressed);
}

#[test]
fn the_bucket_refills_at_the_rate() {
    let mut limiter = limiter(2, 1);
    let def = &catalogue::SERVER_UNREACHABLE;
    assert_eq!(limiter.admit(def, 0), Admission::Admitted);
    assert_eq!(limiter.admit(def, SECOND / 2 - 1), Admission::Suppressed);
    assert_eq!(limiter.admit(def, SECOND / 2), Admission::Admitted);
}

#[test]
fn subsystems_have_separate_buckets() {
    let mut limiter = limiter(1, 1);
    assert_eq!(
        limiter.admit(&catalogue::SERVER_UNREACHABLE, 0),
        Admission::Admitted
    );
    assert_eq!(
        limiter.admit(&catalogue::SERVER_UNREACHABLE, 0),
        Admission::Suppressed
    );
    assert_eq!(
        limiter.admit(&catalogue::VOLUME_OFFLINE, 0),
        Admission::Admitted
    );
    assert_eq!(limiter.suppressed(Subsystem::Server), 1);
    assert_eq!(limiter.suppressed(Subsystem::Volume), 0);
}

#[test]
fn audit_is_never_suppressed() {
    let mut limiter = limiter(1, 1);
    for _ in 0..1_000 {
        assert_eq!(limiter.admit(&catalogue::AUDIT_GAP, 0), Admission::Admitted);
    }
    assert_eq!(limiter.suppressed(Subsystem::Daemon), 0);
}

#[test]
fn suppressed_events_are_counted() {
    let mut limiter = limiter(1, 2);
    let mut refused = 0_u64;
    for _ in 0..10 {
        if limiter.admit(&catalogue::SERVER_UNREACHABLE, 0) == Admission::Suppressed {
            refused += 1;
        }
    }
    assert_eq!(refused, 8);
    assert_eq!(limiter.suppressed(Subsystem::Server), 8);
}

#[test]
fn an_instant_that_overflows_is_suppressed_and_counted() {
    let mut limiter = limiter(1, 2);
    let def = &catalogue::SERVER_UNREACHABLE;
    assert_eq!(limiter.admit(def, u64::MAX), Admission::Suppressed);
    assert_eq!(limiter.suppressed(Subsystem::Server), 1);
}

#[test]
fn take_suppressed_summarises_and_clears() {
    let mut limiter = limiter(1, 1);
    for _ in 0..4 {
        limiter.admit(&catalogue::VOLUME_OFFLINE, 0);
    }
    let events = limiter.take_suppressed(Role::Daemon).unwrap();
    assert_eq!(events.len(), 1);
    let event = &events[0];
    assert_eq!(event.def().name, "events.suppressed");
    assert_eq!(event.value(0), Some(&Value::Static("volume")));
    assert_eq!(event.value(1), Some(&Value::Unsigned(3)));
    assert_eq!(limiter.suppressed(Subsystem::Volume), 0);
    assert_eq!(limiter.take_suppressed(Role::Daemon).unwrap(), []);
}

#[test]
fn config_refuses_zero_and_values_over_the_maximum() {
    assert_eq!(LimitConfig::new(0, 1), Err(LimitError::Rate));
    assert_eq!(LimitConfig::new(LIMIT_MAX + 1, 1), Err(LimitError::Rate));
    assert_eq!(LimitConfig::new(1, 0), Err(LimitError::Burst));
    assert_eq!(LimitConfig::new(1, LIMIT_MAX + 1), Err(LimitError::Burst));
    assert!(LimitConfig::new(LIMIT_MAX, LIMIT_MAX).is_ok());
}

#[test]
fn the_largest_limit_admits_its_burst() {
    let mut limiter = limiter(LIMIT_MAX, LIMIT_MAX);
    let def = &catalogue::SERVER_UNREACHABLE;
    for _ in 0..LIMIT_MAX {
        assert_eq!(limiter.admit(def, 0), Admission::Admitted);
    }
    assert_eq!(limiter.admit(def, 0), Admission::Suppressed);
}

#[test]
fn loss_counters_record_and_take() {
    let mut counters = LossCounters::default();
    counters.record(Loss::QueueFull);
    counters.record(Loss::QueueFull);
    counters.record(Loss::Truncated);
    assert_eq!(counters.get(Loss::QueueFull), 2);
    let events = counters.take_lost(Role::Cli).unwrap();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].value(0), Some(&Value::Static("queue full")));
    assert_eq!(events[0].value(1), Some(&Value::Unsigned(2)));
    assert_eq!(
        events[1].value(0),
        Some(&Value::Static("truncated, delivered in part"))
    );
    for loss in Loss::ALL {
        assert_eq!(counters.get(loss), 0);
    }
}
