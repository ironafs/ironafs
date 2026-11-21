// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Rate limiting with the instant read through the `Clock` seam
//! (`docs/specs/events.md` §5.6 and §10.5, point 2).

use core::time::Duration;

use ironafs_events::{Admission, EventDef, LimitConfig, RateLimiter, Role, Subsystem, catalogue};
use ironafs_test_support::{Property, SeededClock, Source};
use ironafs_time::Clock;

#[test]
fn a_flood_admits_the_burst_and_counts_the_rest() {
    let clock = SeededClock::from_seed(7);
    let mut limiter = RateLimiter::new(LimitConfig::default());
    let now = clock.boot().unwrap().as_nanos();
    let admitted = (0..10_000)
        .filter(|_| limiter.admit(&catalogue::VOLUME_OFFLINE, now) == Admission::Admitted)
        .count();
    assert_eq!(admitted, 50);
    assert_eq!(limiter.suppressed(Subsystem::Volume), 9_950);

    clock.advance(Duration::from_secs(1)).unwrap();
    let later = clock.boot().unwrap().as_nanos();
    let admitted = (0..100)
        .filter(|_| limiter.admit(&catalogue::VOLUME_OFFLINE, later) == Admission::Admitted)
        .count();
    assert_eq!(admitted, 10);

    let summary = limiter.take_suppressed(Role::Daemon).unwrap();
    assert_eq!(summary.len(), 1);
    assert_eq!(limiter.suppressed(Subsystem::Volume), 0);
}

const DEFINITIONS: [&EventDef; 4] = [
    &catalogue::SERVER_UNREACHABLE,
    &catalogue::VOLUME_OFFLINE,
    &catalogue::EVENTS_LOST,
    &catalogue::AUDIT_GAP,
];

#[test]
fn every_refusal_is_counted() {
    Property::new("the suppressed counts equal the refusals").check(
        |source: &mut Source| {
            let config = (source.u64_in(1..=20), source.u64_in(1..=20));
            let steps = source.vec_of(0..=200, |source| {
                (source.usize_in(0..=3), source.u64_in(0..=300_000_000))
            });
            (config, steps)
        },
        |((rate, burst), steps)| {
            let config = LimitConfig::new(
                u32::try_from(*rate).unwrap(),
                u32::try_from(*burst).unwrap(),
            )
            .unwrap();
            let mut limiter = RateLimiter::new(config);
            let mut now = 0_u64;
            let mut refused = [0_u64; Subsystem::COUNT];
            for (which, advance) in steps {
                now += advance;
                let def = DEFINITIONS[*which];
                if limiter.admit(def, now) == Admission::Suppressed {
                    assert_ne!(
                        def.stream,
                        ironafs_events::Stream::Audit,
                        "audit was suppressed"
                    );
                    refused[def.subsystem.index()] += 1;
                }
            }
            for subsystem in Subsystem::ALL {
                assert_eq!(
                    limiter.suppressed(subsystem),
                    refused[subsystem.index()],
                    "{subsystem:?}"
                );
            }
        },
    );
}
