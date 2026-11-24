// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The counter's contract: unavailable until the first move, strictly
//! increasing by one, every consumer told of every move, and stopped on
//! exhaustion, on request and on drop (docs/specs/network-generation.md,
//! 5.2 and 5.3).

use alloc::string::ToString;

use super::{NetworkGeneration, Publisher, Stopped, UNAVAILABLE, Unavailable};
use crate::consumer::probe::probes;
use crate::consumer::{Cause, Change, Loss};

#[test]
fn reader_is_unavailable_before_the_first_move() {
    let (consumers, _) = probes();
    let (_publisher, reader) = Publisher::new(consumers);
    assert_eq!(reader.current(), Err(Unavailable));
}

#[test]
fn advance_publishes_one_then_two() {
    let (consumers, _) = probes();
    let (mut publisher, reader) = Publisher::new(consumers);
    assert_eq!(publisher.advance(Cause::Snapshot), Ok(NetworkGeneration(1)));
    assert_eq!(reader.current(), Ok(NetworkGeneration(1)));
    assert_eq!(
        publisher.advance(Cause::Changed(Change::Route)),
        Ok(NetworkGeneration(2))
    );
    assert_eq!(reader.current(), Ok(NetworkGeneration(2)));
}

#[test]
fn advance_tells_every_consumer_the_new_generation() {
    let (consumers, seen) = probes();
    let (mut publisher, _reader) = Publisher::new(consumers);
    publisher.advance(Cause::Changed(Change::Route)).unwrap();
    publisher.advance(Cause::Lost(Loss::Overflow)).unwrap();
    for each in &seen {
        assert_eq!(each.moves(), 2);
        assert_eq!(each.last(), 2);
        assert_eq!(each.losses(), 1);
    }
}

#[test]
fn every_hook_reads_the_old_generation_during_its_call() {
    let (consumers, seen) = probes();
    let (mut publisher, _reader) = Publisher::new(consumers);
    for _ in 0..3 {
        publisher.advance(Cause::Changed(Change::Route)).unwrap();
    }
    for each in &seen {
        assert_eq!(each.moves(), 3);
        assert_eq!(each.late(), 0);
    }
}

#[test]
fn advance_at_the_last_value_stops_instead_of_publishing_the_marker() {
    let (consumers, seen) = probes();
    let (mut publisher, reader) = Publisher::new(consumers);
    publisher.value = UNAVAILABLE - 2;
    assert_eq!(
        publisher.advance(Cause::Changed(Change::Route)),
        Ok(NetworkGeneration(UNAVAILABLE - 1))
    );
    assert_eq!(
        publisher.advance(Cause::Changed(Change::Route)),
        Err(Stopped)
    );
    assert_eq!(reader.current(), Err(Unavailable));
    assert_eq!(
        publisher.advance(Cause::Changed(Change::Route)),
        Err(Stopped)
    );
    for each in &seen {
        assert_eq!(each.moves(), 1);
        assert_eq!(each.stops(), 1);
    }
}

#[test]
fn stop_tells_every_consumer_once_and_reads_fail() {
    let (consumers, seen) = probes();
    let (mut publisher, reader) = Publisher::new(consumers);
    publisher.advance(Cause::Snapshot).unwrap();
    publisher.stop();
    publisher.stop();
    assert_eq!(reader.current(), Err(Unavailable));
    assert_eq!(
        publisher.advance(Cause::Changed(Change::Route)),
        Err(Stopped)
    );
    for each in &seen {
        assert_eq!(each.stops(), 1);
    }
}

#[test]
fn dropping_the_publisher_stops_the_source() {
    let (consumers, seen) = probes();
    let (mut publisher, reader) = Publisher::new(consumers);
    publisher.advance(Cause::Snapshot).unwrap();
    drop(publisher);
    assert_eq!(reader.current(), Err(Unavailable));
    for each in &seen {
        assert_eq!(each.stops(), 1);
    }
}

#[test]
fn unavailable_says_so_in_lowercase() {
    assert_eq!(
        Unavailable.to_string(),
        "the network generation is unavailable"
    );
}

#[test]
fn a_withheld_publisher_tells_every_move_but_publishes_none_until_released() {
    let (consumers, seen) = probes();
    let (mut publisher, reader) = Publisher::new(consumers);
    publisher.advance(Cause::Snapshot).unwrap();
    publisher.withhold();
    assert_eq!(reader.current(), Err(Unavailable));
    publisher.advance(Cause::Lost(Loss::Overflow)).unwrap();
    publisher.advance(Cause::Changed(Change::Route)).unwrap();
    assert_eq!(reader.current(), Err(Unavailable));
    assert_eq!(publisher.release(Cause::Snapshot), Ok(NetworkGeneration(4)));
    assert_eq!(reader.current(), Ok(NetworkGeneration(4)));
    for each in &seen {
        assert_eq!((each.moves(), each.late()), (4, 0));
    }
}

#[test]
fn halt_makes_reads_fail_and_tells_no_consumer() {
    let (consumers, seen) = probes();
    let (mut publisher, reader) = Publisher::new(consumers);
    publisher.advance(Cause::Snapshot).unwrap();
    publisher.halt();
    assert_eq!(reader.current(), Err(Unavailable));
    assert_eq!(
        publisher.advance(Cause::Changed(Change::Route)),
        Err(Stopped)
    );
    drop(publisher);
    for each in &seen {
        assert_eq!((each.moves(), each.stops()), (1, 0));
    }
}
