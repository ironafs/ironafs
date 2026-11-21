// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for counters, histograms and health (`docs/specs/events.md`
//! §5.7).

use alloc::vec::Vec;

use super::{Counter, Health, HealthLevel, HealthReason, Histogram, HistogramError};

#[test]
fn a_counter_saturates_at_the_maximum() {
    let mut counter = Counter::new();
    counter.add(u64::MAX - 1);
    counter.increment();
    counter.increment();
    assert_eq!(counter.get(), u64::MAX);
    assert_eq!(counter.take(), u64::MAX);
    assert_eq!(counter.get(), 0);
}

#[test]
fn a_histogram_refuses_bounds_that_do_not_increase() {
    assert_eq!(
        Histogram::new([5, 5]).err(),
        Some(HistogramError::NotIncreasing)
    );
    assert_eq!(
        Histogram::new([6, 5]).err(),
        Some(HistogramError::NotIncreasing)
    );
    assert_eq!(
        Histogram::<0>::new([]).err(),
        Some(HistogramError::BucketCount)
    );
    assert_eq!(
        Histogram::new([0_u64; 33]).err(),
        Some(HistogramError::BucketCount)
    );
}

#[test]
fn a_value_on_a_bound_goes_in_that_bucket() {
    let mut histogram = Histogram::new([10, 20]).unwrap();
    histogram.observe(0);
    histogram.observe(10);
    histogram.observe(11);
    histogram.observe(20);
    histogram.observe(21);
    assert_eq!(histogram.bucket_counts(), [2, 2]);
    assert_eq!(histogram.overflow(), 1);
    assert_eq!(histogram.count(), 5);
}

#[test]
fn a_histogram_sum_saturates() {
    let mut histogram = Histogram::new([1]).unwrap();
    histogram.observe(u64::MAX);
    histogram.observe(u64::MAX);
    assert_eq!(histogram.sum(), u64::MAX);
    assert_eq!(histogram.count(), 2);
}

#[test]
fn health_is_the_worst_level_of_its_reasons() {
    let mut health = Health::default();
    assert_eq!(health.level(), HealthLevel::Healthy);
    health.raise(HealthReason::EventsLost);
    assert_eq!(health.level(), HealthLevel::Degraded);
    health.raise(HealthReason::StateUnusable);
    assert_eq!(health.level(), HealthLevel::Failed);
    health.clear(HealthReason::StateUnusable);
    assert_eq!(health.level(), HealthLevel::Degraded);
}

#[test]
fn health_lists_its_reasons_in_order() {
    let mut health = Health::default();
    for reason in HealthReason::ALL.into_iter().rev() {
        health.raise(reason);
    }
    assert_eq!(
        health.reasons().collect::<Vec<_>>(),
        HealthReason::ALL.to_vec()
    );
}
