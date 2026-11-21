// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The witness reads named events in the window, on the window's object.

use super::{Order, Race, Report};

const RACE: Race = Race {
    open: "open",
    close: "close",
    first: "a",
    second: "b",
    floor: 1,
};

#[test]
fn witness_orders_the_meeting_events_inside_the_window() {
    let ab = "d1 t0 op open x\nd2 t0 op a x\nd3 t1 op b x\nd4 t0 op close x\n";
    let ba = "d1 t0 op open x\nd2 t1 op b x\nd3 t0 op a x\nd4 t0 op close x\n";
    assert_eq!(RACE.witness(ab), Some(Order::FirstThenSecond));
    assert_eq!(RACE.witness(ba), Some(Order::SecondThenFirst));
}

#[test]
fn a_meeting_event_outside_the_window_is_not_exercised() {
    let before = "d1 t1 op b x\nd2 t0 op open x\nd3 t0 op a x\nd4 t0 op close x\n";
    let after = "d1 t0 op open x\nd2 t0 op a x\nd3 t0 op close x\nd4 t1 op b x\n";
    let unclosed = "d1 t0 op open x\nd2 t0 op a x\nd3 t1 op b x\n";
    for trace in [before, after, unclosed] {
        assert_eq!(RACE.witness(trace), None, "{trace}");
    }
}

#[test]
fn meeting_events_on_different_objects_are_not_exercised() {
    let other = "d1 t0 op open x\nd2 t0 op a x\nd3 t1 op b y\nd4 t0 op close x\n";
    assert_eq!(RACE.witness(other), None);
    let closed_on_other = "d1 t0 op open x\nd2 t0 op a x\nd3 t1 op b x\nd4 t0 op close y\n";
    assert_eq!(RACE.witness(closed_on_other), None);
}

#[test]
fn check_fails_an_order_below_its_floor() {
    let race = Race { floor: 2, ..RACE };
    let report = Report {
        first_then_second: 5,
        second_then_first: 1,
        not_exercised: 3,
    };
    assert_eq!(report.check(&race, &[Order::FirstThenSecond]), Ok(()));
    let failed = report
        .check(&race, &[Order::FirstThenSecond, Order::SecondThenFirst])
        .unwrap_err();
    assert_eq!(failed.order, Order::SecondThenFirst);
    assert_eq!(failed.witnessed, 1);
}
