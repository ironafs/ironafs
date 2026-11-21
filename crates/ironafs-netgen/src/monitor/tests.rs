// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The monitor's states and its answer to each event and each failure
//! (docs/specs/network-generation.md, 5.7 and section 7), on scripted
//! datagrams, with a probe standing in for each consumer.

use alloc::format;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;

use super::{Monitor, Request, SNAPSHOT_ATTEMPTS};
use crate::consumer::probe::{Seen, loss_code, probes};
use crate::generation::Stopped;
use crate::netlink::Dump;
use crate::netlink::build::{self, AF_INET, AF_INET6};
use crate::{Loss, Reader, Unavailable};

/// `NLM_F_MULTI`, `NLM_F_MULTI | NLM_F_DUMP_INTR`.
const MULTI: u16 = 0x2;
const INTERRUPTED: u16 = 0x12;

fn done(sequence: u32, flags: u16) -> Vec<u8> {
    build::message(3, flags, sequence, &[0; 4])
}

fn up_link(sequence: u32, index: i32) -> Vec<u8> {
    build::link_with(16, sequence, index, 0x0001_0041, |body| {
        build::attribute(body, 16, &[6]);
    })
}

fn v4_address(sequence: u32, kind: u16, flags: u8) -> Vec<u8> {
    build::address_with(kind, sequence, AF_INET, 2, 24, flags, |body| {
        build::attribute(body, 2, &[192, 0, 2, 1]);
    })
}

fn v6_address(sequence: u32, kind: u16, flags: u8) -> Vec<u8> {
    build::address_with(kind, sequence, AF_INET6, 2, 64, flags, |body| {
        build::attribute(
            body,
            1,
            &[0x20, 1, 0xd, 0xb8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
        );
    })
}

/// Answers the monitor's outstanding link and address dumps with one link
/// and two addresses.
fn answer_snapshot(monitor: &mut Monitor) {
    let Some(links) = monitor.request() else {
        panic!("no snapshot outstanding");
    };
    assert_eq!(links.dump, Dump::Links);
    let mut datagram = up_link(links.sequence, 2);
    datagram.extend(done(links.sequence, MULTI));
    monitor.reply(&datagram).unwrap();
    let Some(addresses) = monitor.request() else {
        panic!("no address dump after the link dump");
    };
    assert_eq!(addresses.dump, Dump::Addresses);
    assert!(links.is_links() && !addresses.is_links());
    assert_ne!(addresses.sequence, links.sequence);
    let mut datagram = v4_address(addresses.sequence, 20, 0x80);
    datagram.extend(v6_address(addresses.sequence, 20, 0x80));
    datagram.extend(done(addresses.sequence, MULTI));
    monitor.reply(&datagram).unwrap();
    assert_eq!(monitor.request(), None);
    let Some(barrier) = monitor.barrier() else {
        panic!("no barrier after the address dump");
    };
    monitor.notification(&ack(barrier.sequence)).unwrap();
    assert_eq!(monitor.barrier(), None);
}

/// The kernel's acknowledgement of a request of `sequence`: `NLMSG_ERROR`
/// with error 0 and the request's header.
fn ack(sequence: u32) -> Vec<u8> {
    let mut body = 0_i32.to_ne_bytes().to_vec();
    body.extend(build::message(1, 5, sequence, &[]));
    build::message(2, 0x100, sequence, &body)
}

/// The kernel's refusal of a request of `sequence`: `NLMSG_ERROR` with
/// `code` and the request's header.
fn refusal(sequence: u32, code: i32) -> Vec<u8> {
    let mut body = code.to_ne_bytes().to_vec();
    body.extend(build::message(18, 0x301, sequence, &[]));
    build::message(2, 0, sequence, &body)
}

/// A monitor whose driver has subscribed, and its reader.
fn fresh(make: impl FnOnce(&Reader) -> crate::Consumers) -> (Monitor, Reader) {
    let (mut monitor, reader) = Monitor::new(make);
    monitor.subscribed();
    (monitor, reader)
}

/// Makes the notify socket again where a loss asked for it.
fn resubscribe(monitor: &mut Monitor) {
    if monitor.needs_subscription() {
        assert_eq!(monitor.request(), None);
        monitor.subscribed();
    }
}

/// A monitor with its first snapshot done, its reader and its probes.
fn started() -> (Monitor, Reader, [Arc<Seen>; 7]) {
    let (make, seen) = probes();
    let (mut monitor, reader) = fresh(make);
    answer_snapshot(&mut monitor);
    (monitor, reader, seen)
}

fn current(reader: &Reader) -> u64 {
    reader.current().unwrap().get()
}

/// The acceptance check (issue #97): after `drive`, which makes `changes`
/// changes, the generation moved at least once per change, every probe
/// was told of every move, and no probe ever read the generation it was
/// being told.
fn every_hook_follows_every_move(
    changes: u64,
    drive: impl FnOnce(&mut Monitor),
) -> Result<(), String> {
    let (mut monitor, reader, seen) = started();
    let before = current(&reader);
    drive(&mut monitor);
    let after = current(&reader);
    if after < before + changes {
        return Err(format!(
            "{changes} changes moved the generation from {before} to {after}"
        ));
    }
    for (index, each) in seen.iter().enumerate() {
        if each.moves() != after || each.last() != after {
            return Err(format!(
                "consumer {index} told {} moves, last {}, of {after}",
                each.moves(),
                each.last()
            ));
        }
        if each.late() != 0 {
            return Err(format!(
                "consumer {index} read a generation before being told"
            ));
        }
    }
    Ok(())
}

#[test]
fn a_new_monitor_asks_for_links_first_and_publishes_nothing() {
    let (make, _) = probes();
    let (mut monitor, reader) = Monitor::new(make);
    // Nothing is asked for until the driver has subscribed.
    assert_eq!(monitor.request(), None);
    assert!(monitor.needs_subscription());
    monitor.subscribed();
    assert!(!monitor.needs_subscription());
    assert!(matches!(
        monitor.request(),
        Some(Request {
            dump: Dump::Links,
            ..
        })
    ));
    assert_eq!(reader.current(), Err(Unavailable));
}

#[test]
fn the_first_snapshot_publishes_generation_one_to_every_consumer() {
    let (_monitor, reader, seen) = started();
    assert_eq!(current(&reader), 1);
    for each in &seen {
        assert_eq!((each.moves(), each.snapshots(), each.late()), (1, 1, 0));
    }
}

#[test]
fn a_request_s_bytes_are_its_dump_with_its_sequence() {
    let (make, _) = probes();
    let (monitor, _reader) = fresh(make);
    let request = monitor.request().unwrap();
    assert_eq!(
        request.bytes(),
        crate::netlink::dump_request(Dump::Links, request.sequence)
    );
}

#[test]
fn every_hook_is_told_of_each_route_rule_nexthop_address_and_link_change() {
    let result = every_hook_follows_every_move(7, |monitor| {
        for kind in [24, 33, 104] {
            monitor
                .notification(&build::message(kind, 0, 0, &[0; 12]))
                .unwrap();
        }
        monitor.notification(&v4_address(0, 21, 0x80)).unwrap();
        monitor.notification(&v4_address(0, 20, 0x80)).unwrap();
        monitor
            .notification(&build::link(16, 2, 0x0001_0040, |_| {}))
            .unwrap();
        monitor
            .notification(&build::link(17, 2, 0, |_| {}))
            .unwrap();
    });
    assert_eq!(result, Ok(()));
}

#[test]
fn a_source_that_never_fires_fails_the_acceptance_check() {
    // Planted: the change happens, but nothing reaches the monitor.
    let result = every_hook_follows_every_move(1, |_monitor| {});
    assert_eq!(
        result,
        Err(String::from("1 changes moved the generation from 1 to 1"))
    );
}

#[test]
fn several_changes_in_one_datagram_move_once_each() {
    let (mut monitor, reader, _) = started();
    let mut datagram = build::message(24, 0, 0, &[0; 12]);
    datagram.extend(build::message(32, 0, 0, &[0; 12]));
    datagram.extend(build::message(25, 0, 0, &[0; 12]));
    monitor.notification(&datagram).unwrap();
    assert_eq!(current(&reader), 4);
}

#[test]
fn an_address_lifetime_refresh_moves_nothing_and_deprecation_moves() {
    let (mut monitor, reader, _) = started();
    let refresh = build::address(20, AF_INET6, 2, 64, 0x80, |body| {
        build::attribute(
            body,
            1,
            &[0x20, 1, 0xd, 0xb8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
        );
        build::attribute(body, 6, &[0xff; 16]);
    });
    monitor.notification(&refresh).unwrap();
    assert_eq!(current(&reader), 1);
    monitor
        .notification(&v6_address(0, 20, 0x80 | 0x20))
        .unwrap();
    assert_eq!(current(&reader), 2);
    monitor
        .notification(&v6_address(0, 20, 0x80 | 0x20))
        .unwrap();
    assert_eq!(current(&reader), 2);
}

#[test]
fn a_new_address_and_a_removed_one_each_move() {
    let (mut monitor, reader, _) = started();
    let other = build::address(20, AF_INET, 2, 24, 0x80, |body| {
        build::attribute(body, 2, &[192, 0, 2, 9]);
    });
    monitor.notification(&other).unwrap();
    assert_eq!(current(&reader), 2);
    monitor.notification(&v4_address(0, 21, 0x80)).unwrap();
    assert_eq!(current(&reader), 3);
    // Removed and back with the same attributes: a change, not a refresh.
    monitor.notification(&v4_address(0, 20, 0x80)).unwrap();
    assert_eq!(current(&reader), 4);
}

#[test]
fn a_link_message_with_the_same_usability_moves_nothing() {
    let (mut monitor, reader, _) = started();
    // The MTU or name changed: same flags, same operational state.
    let same = build::link(16, 2, 0x0001_0041 | 0x1002, |body| {
        build::attribute(body, 16, &[6]);
        build::attribute(body, 4, &1400_u32.to_ne_bytes());
    });
    monitor.notification(&same).unwrap();
    assert_eq!(current(&reader), 1);
    let dormant = build::link(16, 2, 0x0001_0041, |body| {
        build::attribute(body, 16, &[5]);
    });
    monitor.notification(&dormant).unwrap();
    assert_eq!(current(&reader), 2);
}

#[test]
fn a_removed_link_takes_its_addresses_out_of_the_table() {
    let (mut monitor, reader, _) = started();
    monitor
        .notification(&build::link(17, 2, 0, |_| {}))
        .unwrap();
    assert_eq!(current(&reader), 2);
    // The same address back on a new link of the same index is new.
    monitor.notification(&v4_address(0, 20, 0x80)).unwrap();
    assert_eq!(current(&reader), 3);
}

#[test]
fn control_and_unknown_messages_on_the_notify_socket_move_nothing() {
    let (mut monitor, reader, _) = started();
    let mut datagram = build::message(1, 0, 0, &[]);
    datagram.extend(build::message(3, 0, 0, &[0; 4]));
    datagram.extend(ack(0));
    datagram.extend(build::message(68, 0, 0, &[0; 8]));
    monitor.notification(&datagram).unwrap();
    assert_eq!(current(&reader), 1);
}

/// The answer to one loss: every hook told of it at once, then a fresh
/// snapshot asked for, whose completion moves again.
#[track_caller]
fn loss_invalidates_then_resnapshots(loss: Loss, cause: impl FnOnce(&mut Monitor)) {
    let (mut monitor, reader, seen) = started();
    cause(&mut monitor);
    // Invalidated at once, and nothing readable until a verified snapshot.
    assert_eq!(reader.current(), Err(Unavailable));
    for each in &seen {
        assert_eq!(each.moves(), 2);
        assert_eq!(each.losses(), 1);
        assert_eq!(each.last_loss(), loss_code(loss));
        assert_eq!(each.late(), 0);
    }
    // A loss on the notify socket makes the driver subscribe again before
    // the snapshot, so no queued record from before the loss is applied
    // to the fresh tables.
    assert_eq!(monitor.needs_subscription(), loss != Loss::Resumed);
    resubscribe(&mut monitor);
    assert!(matches!(
        monitor.request(),
        Some(Request {
            dump: Dump::Links,
            ..
        })
    ));
    answer_snapshot(&mut monitor);
    assert_eq!(current(&reader), 3);
    for each in &seen {
        assert_eq!(each.snapshots(), 2);
    }
}

#[test]
fn a_planted_overflow_invalidates_every_consumer_then_takes_a_fresh_snapshot() {
    loss_invalidates_then_resnapshots(Loss::Overflow, |monitor| {
        monitor.lost(Loss::Overflow).unwrap();
    });
}

#[test]
fn a_planted_truncation_invalidates_every_consumer_then_takes_a_fresh_snapshot() {
    loss_invalidates_then_resnapshots(Loss::Truncated, |monitor| {
        monitor.lost(Loss::Truncated).unwrap();
    });
}

#[test]
fn a_resume_invalidates_every_consumer_then_takes_a_fresh_snapshot() {
    loss_invalidates_then_resnapshots(Loss::Resumed, |monitor| {
        monitor.lost(Loss::Resumed).unwrap();
    });
}

#[test]
fn a_malformed_notification_is_a_loss() {
    loss_invalidates_then_resnapshots(Loss::Malformed, |monitor| {
        monitor
            .notification(&build::message(20, 0, 0, &[0; 3]))
            .unwrap();
    });
}

#[test]
fn an_overrun_message_is_a_loss() {
    loss_invalidates_then_resnapshots(Loss::Malformed, |monitor| {
        monitor.notification(&build::message(4, 0, 0, &[])).unwrap();
    });
}

#[test]
fn messages_after_a_malformed_one_are_not_applied() {
    let (mut monitor, reader, seen) = started();
    let mut datagram = build::message(24, 0, 0, &[0; 12]);
    datagram.extend(build::pad(build::message(16, 0, 0, &[0; 3])));
    datagram.extend(build::message(24, 0, 0, &[0; 12]));
    monitor.notification(&datagram).unwrap();
    // The first route moved it, the bad link is a loss; the last route is
    // not read.
    assert_eq!(reader.current(), Err(Unavailable));
    assert_eq!(seen[0].moves(), 3);
}

#[test]
fn a_dump_reply_with_another_sequence_is_ignored() {
    let (make, _) = probes();
    let (mut monitor, reader) = fresh(make);
    let links = monitor.request().unwrap();
    let stale = links.sequence.wrapping_sub(1);
    let mut datagram = up_link(stale, 9);
    datagram.extend(done(stale, MULTI));
    monitor.reply(&datagram).unwrap();
    assert_eq!(monitor.request(), Some(links));
    assert_eq!(reader.current(), Err(Unavailable));
}

#[test]
fn an_interrupted_dump_is_a_loss_and_starts_again() {
    let (mut monitor, reader, seen) = started();
    monitor.lost(Loss::Overflow).unwrap();
    resubscribe(&mut monitor);
    let links = monitor.request().unwrap();
    monitor.reply(&done(links.sequence, INTERRUPTED)).unwrap();
    assert_eq!(reader.current(), Err(Unavailable));
    assert_eq!(seen[0].moves(), 3);
    assert_eq!(seen[0].last_loss(), loss_code(Loss::Interrupted));
    let again = monitor.request().unwrap();
    assert_eq!(again.dump, Dump::Links);
    assert_ne!(again.sequence, links.sequence);
}

#[test]
fn a_dump_answered_with_an_error_is_refused_and_starts_again() {
    let (mut monitor, reader, seen) = started();
    monitor.lost(Loss::Overflow).unwrap();
    resubscribe(&mut monitor);
    let links = monitor.request().unwrap();
    monitor.reply(&refusal(links.sequence, -16)).unwrap();
    assert_eq!(reader.current(), Err(Unavailable));
    assert_eq!(seen[0].moves(), 3);
    assert_eq!(seen[0].last_loss(), loss_code(Loss::Refused));
    // A dump that fails part-way ends in `NLMSG_DONE` with its error.
    let links = monitor.request().unwrap();
    monitor
        .reply(&build::message(
            3,
            MULTI,
            links.sequence,
            &(-12_i32).to_ne_bytes(),
        ))
        .unwrap();
    assert_eq!(seen[0].moves(), 4);
    assert_eq!(seen[0].last_loss(), loss_code(Loss::Refused));
    assert_ne!(monitor.request().unwrap().sequence, links.sequence);
}

#[test]
fn a_malformed_or_truncated_dump_starts_again() {
    let (mut monitor, reader, seen) = started();
    monitor.lost(Loss::Overflow).unwrap();
    resubscribe(&mut monitor);
    let links = monitor.request().unwrap();
    monitor
        .reply(&build::message(16, 0, links.sequence, &[0; 3]))
        .unwrap();
    assert_eq!(seen[0].last_loss(), loss_code(Loss::Malformed));
    monitor.dump_failed(Loss::Truncated).unwrap();
    assert_eq!(seen[0].last_loss(), loss_code(Loss::Truncated));
    assert_eq!(reader.current(), Err(Unavailable));
    assert_eq!(seen[0].moves(), 4);
}

#[test]
fn eight_failed_snapshots_in_a_row_stop_the_source() {
    let (make, seen) = probes();
    let (mut monitor, reader) = fresh(make);
    for _ in 1..SNAPSHOT_ATTEMPTS {
        monitor.dump_failed(Loss::Failed).unwrap();
    }
    assert_eq!(monitor.dump_failed(Loss::Failed), Err(Stopped));
    assert_eq!(reader.current(), Err(Unavailable));
    assert_eq!(monitor.request(), None);
    assert_eq!(monitor.lost(Loss::Overflow), Err(Stopped));
    for each in &seen {
        assert_eq!(each.stops(), 1);
    }
}

#[test]
fn a_completed_snapshot_resets_the_count_of_failures() {
    let (make, _) = probes();
    let (mut monitor, reader) = fresh(make);
    for _ in 1..SNAPSHOT_ATTEMPTS {
        monitor.dump_failed(Loss::Failed).unwrap();
    }
    answer_snapshot(&mut monitor);
    for _ in 1..SNAPSHOT_ATTEMPTS {
        monitor.dump_failed(Loss::Failed).unwrap();
    }
    // Not stopped: the eighth snapshot is still asked for, and completes.
    answer_snapshot(&mut monitor);
    assert!(reader.current().is_ok());
}

#[test]
fn changes_before_the_first_snapshot_publish_nothing_until_it_completes() {
    let (make, seen) = probes();
    let (mut monitor, reader) = fresh(make);
    monitor
        .notification(&build::message(24, 0, 0, &[0; 12]))
        .unwrap();
    monitor.lost(Loss::Overflow).unwrap();
    resubscribe(&mut monitor);
    assert_eq!(reader.current(), Err(Unavailable));
    answer_snapshot(&mut monitor);
    assert_eq!(current(&reader), 1);
    assert_eq!(seen[0].moves(), 1);
}

#[test]
fn completion_moves_even_when_the_tables_are_unchanged() {
    // A change after the subscription that the dump already shows is then
    // read as a refresh: only the completion's move covers it.
    let (mut monitor, reader, _) = started();
    monitor.lost(Loss::Resumed).unwrap();
    answer_snapshot(&mut monitor);
    monitor.notification(&v4_address(0, 20, 0x80)).unwrap();
    assert_eq!(current(&reader), 3);
}

#[test]
fn notifications_during_a_snapshot_still_move_the_generation() {
    let (mut monitor, reader, seen) = started();
    monitor.lost(Loss::Overflow).unwrap();
    resubscribe(&mut monitor);
    monitor
        .notification(&build::message(24, 0, 0, &[0; 12]))
        .unwrap();
    // The consumers are told, but nothing is readable before the snapshot.
    assert_eq!(reader.current(), Err(Unavailable));
    assert_eq!(seen[0].moves(), 3);
    answer_snapshot(&mut monitor);
    assert_eq!(current(&reader), 4);
}

#[test]
fn a_table_over_its_cap_treats_every_message_of_its_kind_as_a_change() {
    let (mut monitor, reader, _) = started();
    monitor.lost(Loss::Overflow).unwrap();
    resubscribe(&mut monitor);
    let links = monitor.request().unwrap();
    let mut datagram = Vec::new();
    for index in 0..=i32::try_from(super::LINKS_MAX).unwrap() {
        datagram.extend(up_link(links.sequence, index + 10));
    }
    datagram.extend(done(links.sequence, MULTI));
    monitor.reply(&datagram).unwrap();
    let addresses = monitor.request().unwrap();
    monitor.reply(&done(addresses.sequence, MULTI)).unwrap();
    let barrier = monitor.barrier().unwrap();
    monitor.notification(&ack(barrier.sequence)).unwrap();
    let before = current(&reader);
    monitor.notification(&up_link(0, 10)).unwrap();
    assert_eq!(current(&reader), before + 1);
}

#[test]
fn stop_tells_every_consumer_and_refuses_everything_after() {
    let (mut monitor, reader, seen) = started();
    monitor.stop();
    assert_eq!(reader.current(), Err(Unavailable));
    assert_eq!(monitor.request(), None);
    assert_eq!(monitor.notification(&[]), Err(Stopped));
    assert_eq!(monitor.reply(&[]), Err(Stopped));
    assert_eq!(monitor.lost(Loss::Resumed), Err(Stopped));
    for each in &seen {
        assert_eq!(each.stops(), 1);
    }
}

#[test]
fn an_overrun_in_a_dump_reply_of_any_sequence_starts_the_snapshot_again() {
    let (mut monitor, _reader, seen) = started();
    monitor.lost(Loss::Resumed).unwrap();
    let links = monitor.request().unwrap();
    monitor
        .reply(&build::message(4, 0, links.sequence.wrapping_add(9), &[]))
        .unwrap();
    assert_eq!(seen[0].last_loss(), loss_code(Loss::Malformed));
    assert_ne!(monitor.request().unwrap().sequence, links.sequence);
}

#[test]
fn a_done_followed_by_malformed_bytes_completes_nothing() {
    let (mut monitor, reader, seen) = started();
    monitor.lost(Loss::Resumed).unwrap();
    let links = monitor.request().unwrap();
    let mut datagram = up_link(links.sequence, 2);
    datagram.extend(done(links.sequence, MULTI));
    monitor.reply(&datagram).unwrap();
    let addresses = monitor.request().unwrap();
    let mut datagram = done(addresses.sequence, MULTI);
    datagram.extend([0; 15]);
    monitor.reply(&datagram).unwrap();
    assert_eq!(reader.current(), Err(Unavailable));
    assert_eq!(seen[0].snapshots(), 1);
    assert_eq!(seen[0].last_loss(), loss_code(Loss::Malformed));
}

#[test]
fn a_link_s_group_master_or_name_changing_moves_the_generation() {
    let (mut monitor, reader, _) = started();
    let with = |attributes: &[(u16, &[u8])]| {
        build::link(16, 2, 0x0001_0041, |body| {
            build::attribute(body, 16, &[6]);
            for (kind, value) in attributes {
                build::attribute(body, *kind, value);
            }
        })
    };
    // The same link, now naming its group, master and name: three
    // changes, then the same message again: none.
    monitor
        .notification(&with(&[(27, &5_u32.to_ne_bytes())]))
        .unwrap();
    assert_eq!(current(&reader), 2);
    let full: [(u16, &[u8]); 3] = [
        (27, &5_u32.to_ne_bytes()),
        (10, &9_u32.to_ne_bytes()),
        (3, b"eth0\0"),
    ];
    monitor.notification(&with(&full[..2])).unwrap();
    assert_eq!(current(&reader), 3);
    monitor.notification(&with(&full)).unwrap();
    assert_eq!(current(&reader), 4);
    monitor.notification(&with(&full)).unwrap();
    assert_eq!(current(&reader), 4);
}

#[test]
fn halt_makes_the_generation_unavailable_without_calling_any_hook() {
    let (mut monitor, reader, seen) = started();
    assert!(!monitor.is_stopped());
    monitor.halt();
    assert!(monitor.is_stopped());
    assert_eq!(reader.current(), Err(Unavailable));
    assert_eq!(monitor.lost(Loss::Resumed), Err(Stopped));
    for each in &seen {
        assert_eq!((each.moves(), each.stops()), (1, 0));
    }
}

#[test]
fn a_snapshot_publishes_only_when_its_barrier_comes_back_on_the_notify_socket() {
    let (mut monitor, reader, seen) = started();
    monitor.lost(Loss::Resumed).unwrap();
    let links = monitor.request().unwrap();
    let mut datagram = up_link(links.sequence, 2);
    datagram.extend(done(links.sequence, MULTI));
    monitor.reply(&datagram).unwrap();
    let addresses = monitor.request().unwrap();
    monitor.reply(&done(addresses.sequence, MULTI)).unwrap();
    // Dumped, but not published: the barrier has not come back.
    assert_eq!(monitor.request(), None);
    let barrier = monitor.barrier().unwrap();
    assert_eq!(barrier.bytes(), request_bytes(1, 5, barrier.sequence));
    assert_eq!(reader.current(), Err(Unavailable));
    // Another acknowledgement is not the barrier.
    monitor
        .notification(&ack(barrier.sequence.wrapping_add(1)))
        .unwrap();
    assert_eq!(reader.current(), Err(Unavailable));
    monitor.notification(&ack(barrier.sequence)).unwrap();
    assert_eq!(current(&reader), 3);
    assert_eq!(seen[0].snapshots(), 2);
}

#[test]
fn a_loss_before_the_barrier_publishes_nothing() {
    let (mut monitor, reader, _) = started();
    monitor.lost(Loss::Resumed).unwrap();
    let links = monitor.request().unwrap();
    monitor.reply(&done(links.sequence, MULTI)).unwrap();
    let addresses = monitor.request().unwrap();
    monitor.reply(&done(addresses.sequence, MULTI)).unwrap();
    let barrier = monitor.barrier().unwrap();
    monitor.lost(Loss::Overflow).unwrap();
    assert_eq!(monitor.barrier(), None);
    monitor.notification(&ack(barrier.sequence)).unwrap();
    assert_eq!(reader.current(), Err(Unavailable));
}

/// Answers the outstanding snapshot's dumps with nothing and returns its
/// barrier, unacknowledged.
fn dump_to_barrier(monitor: &mut Monitor) -> u32 {
    while let Some(dump) = monitor.request() {
        monitor.reply(&done(dump.sequence, MULTI)).unwrap();
    }
    monitor.barrier().unwrap().sequence
}

#[test]
fn a_link_or_address_notification_during_a_snapshot_has_it_taken_again() {
    for phase in 0..3 {
        let (mut monitor, reader, seen) = started();
        monitor.lost(Loss::Resumed).unwrap();
        let links = monitor.request().unwrap();
        if phase > 0 {
            monitor.reply(&done(links.sequence, MULTI)).unwrap();
        }
        if phase > 1 {
            let addresses = monitor.request().unwrap();
            monitor.reply(&done(addresses.sequence, MULTI)).unwrap();
            assert!(monitor.barrier().is_some());
        }
        let outstanding = monitor.request();
        monitor.notification(&v4_address(0, 21, 0x80)).unwrap();
        // The snapshot runs on: one move for the change, and no loss.
        assert_eq!(monitor.request(), outstanding, "{phase}");
        assert_eq!(seen[0].moves(), 3);
        assert_eq!(seen[0].losses(), 1);
        // Its barrier comes back, and it is taken again, not published.
        let barrier = if phase > 1 {
            monitor.barrier().unwrap().sequence
        } else {
            dump_to_barrier(&mut monitor)
        };
        monitor.notification(&ack(barrier)).unwrap();
        assert_eq!(monitor.barrier(), None);
        let again = monitor.request().unwrap();
        assert!(again.is_links());
        assert_ne!(again.sequence, links.sequence);
        assert_eq!(reader.current(), Err(Unavailable));
        assert_eq!((seen[0].moves(), seen[0].losses()), (3, 1));
        answer_snapshot(&mut monitor);
        assert_eq!(current(&reader), 4);
    }
}

#[test]
fn churn_during_snapshots_never_stops_the_source() {
    let (mut monitor, reader, seen) = started();
    monitor.lost(Loss::Resumed).unwrap();
    // More dirty snapshots than the failures that stop the source, each
    // with more notifications than that.
    for round in 0..=SNAPSHOT_ATTEMPTS {
        let barrier = dump_to_barrier(&mut monitor);
        for _ in 0..=SNAPSHOT_ATTEMPTS {
            monitor.notification(&up_link(0, 2)).unwrap();
        }
        monitor.notification(&ack(barrier)).unwrap();
        assert!(!monitor.is_stopped(), "{round}");
    }
    assert_eq!(seen[0].losses(), 1);
    answer_snapshot(&mut monitor);
    assert!(reader.current().is_ok());
}

#[test]
fn a_barrier_refused_or_acknowledged_for_another_request_fails_the_snapshot() {
    for answer in [refusal(0, -105), {
        let mut body = 0_i32.to_ne_bytes().to_vec();
        body.extend(build::message(18, 5, 0, &[]));
        build::message(2, 0x100, 0, &body)
    }] {
        let (mut monitor, reader, seen) = started();
        monitor.lost(Loss::Resumed).unwrap();
        let barrier = dump_to_barrier(&mut monitor);
        let mut datagram = answer.clone();
        datagram[8..12].copy_from_slice(&barrier.to_ne_bytes());
        monitor.notification(&datagram).unwrap();
        assert_eq!(reader.current(), Err(Unavailable));
        assert_eq!(seen[0].last_loss(), loss_code(Loss::Refused));
        assert_eq!(monitor.barrier(), None);
        assert!(monitor.request().unwrap().is_links());
    }
}

#[test]
fn a_barrier_s_acknowledgement_before_bad_bytes_publishes_nothing() {
    let (mut monitor, reader, seen) = started();
    monitor.lost(Loss::Resumed).unwrap();
    let barrier = dump_to_barrier(&mut monitor);
    let mut datagram = ack(barrier);
    datagram.extend(build::message(4, 0, 0, &[]));
    monitor.notification(&datagram).unwrap();
    assert_eq!(reader.current(), Err(Unavailable));
    assert_eq!(seen[0].snapshots(), 1);
    assert_eq!(seen[0].last_loss(), loss_code(Loss::Malformed));
}

#[test]
fn nothing_after_the_link_dump_s_terminator_is_applied_under_the_next_sequence() {
    let (mut monitor, reader, _) = started();
    monitor.lost(Loss::Resumed).unwrap();
    let links = monitor.request().unwrap();
    // The address dump's terminator, before its request was sent.
    let mut datagram = done(links.sequence, MULTI);
    datagram.extend(done(super::next_after(links.sequence), MULTI));
    monitor.reply(&datagram).unwrap();
    let addresses = monitor.request().unwrap();
    assert_eq!(addresses.dump, Dump::Addresses);
    assert_eq!(monitor.barrier(), None);
    assert_eq!(reader.current(), Err(Unavailable));
}

#[test]
fn a_snapshot_s_tables_hold_what_its_dumps_returned() {
    let (monitor, _reader, _) = started();
    assert_eq!(monitor.links.entries.len(), 1);
    assert_eq!(monitor.addresses.entries.len(), 2);
}

#[test]
fn a_route_notification_during_a_snapshot_keeps_it() {
    let (mut monitor, _reader, seen) = started();
    monitor.lost(Loss::Resumed).unwrap();
    let links = monitor.request().unwrap();
    monitor
        .notification(&build::message(24, 0, 0, &[0; 12]))
        .unwrap();
    assert_eq!(monitor.request(), Some(links));
    assert_eq!(seen[0].last_loss(), loss_code(Loss::Resumed));
}

#[test]
fn an_error_or_any_message_after_the_terminator_of_the_dump_refuses_it() {
    let (mut monitor, reader, seen) = started();
    monitor.lost(Loss::Resumed).unwrap();
    // The link dump: its terminator, then an error of the same sequence.
    let links = monitor.request().unwrap();
    let mut datagram = done(links.sequence, MULTI);
    datagram.extend(refusal(links.sequence, -16));
    monitor.reply(&datagram).unwrap();
    assert_eq!(seen[0].last_loss(), loss_code(Loss::Refused));
    assert!(monitor.request().unwrap().is_links());
    // The address dump: its terminator, then a link of the same sequence.
    let links = monitor.request().unwrap();
    monitor.reply(&done(links.sequence, MULTI)).unwrap();
    let addresses = monitor.request().unwrap();
    let mut datagram = done(addresses.sequence, MULTI);
    datagram.extend(up_link(addresses.sequence, 4));
    monitor.reply(&datagram).unwrap();
    assert_eq!(seen[0].last_loss(), loss_code(Loss::Malformed));
    assert_eq!(monitor.barrier(), None);
    assert_eq!(reader.current(), Err(Unavailable));
}

#[test]
fn notifications_say_what_changed() {
    let (mut monitor, _reader, seen) = started();
    for (message, change) in [
        (build::message(24, 0, 0, &[0; 12]), 3),
        (build::message(32, 0, 0, &[0; 12]), 4),
        (build::message(104, 0, 0, &[0; 12]), 5),
        (v4_address(0, 21, 0x80), 2),
        (build::link(17, 2, 0, |_| {}), 1),
    ] {
        monitor.notification(&message).unwrap();
        assert_eq!(seen[0].last_change(), change);
    }
}

/// A request's header alone: `kind`, `flags`, `sequence`, port id 0.
fn request_bytes(kind: u16, flags: u16, sequence: u32) -> [u8; 16] {
    let mut bytes = [0; 16];
    bytes[..4].copy_from_slice(&16_u32.to_ne_bytes());
    bytes[4..6].copy_from_slice(&kind.to_ne_bytes());
    bytes[6..8].copy_from_slice(&flags.to_ne_bytes());
    bytes[8..12].copy_from_slice(&sequence.to_ne_bytes());
    bytes
}

#[test]
fn a_wake_is_a_no_op_request_with_an_acknowledgement_and_sequence_zero() {
    assert_eq!(super::wake(), request_bytes(1, 5, 0));
}
