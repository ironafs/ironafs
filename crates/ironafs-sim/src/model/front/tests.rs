// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The model front end applies advances and writes replies one item at a
//! time, discarding a reply older than the epoch it has applied.

use super::{FRONT_CAP, FrontEnd, FrontStream, Item, Step};
use crate::model::check::{BREAK_ORDER, BreakOrder};
use crate::model::types::Fid;

fn fid() -> Fid {
    Fid::new(100, 2, 1)
}

fn reply(epoch: u64, version: u64) -> Item {
    Item::Reply {
        epoch,
        fid: fid(),
        version,
    }
}

fn advance(epoch: u64, version: u64) -> Item {
    Item::Advance {
        epoch,
        fid: fid(),
        version,
    }
}

#[test]
fn a_reply_older_than_the_applied_epoch_is_discarded() {
    let mut front = FrontEnd::new();
    let mut order = BreakOrder::default();
    assert_eq!(front.step(&reply(0, 1), &mut order), Step::Written);
    assert_eq!(front.step(&advance(1, 1), &mut order), Step::Applied);
    assert_eq!(front.applied(), 1);
    assert_eq!(front.step(&reply(0, 1), &mut order), Step::Discarded);
    assert_eq!(front.step(&reply(1, 2), &mut order), Step::Written);
    assert_eq!(order.check(), Ok(()));
}

#[test]
fn without_the_comparison_a_stale_reply_is_written_and_the_order_breaks() {
    let mut front = FrontEnd::new();
    front.plant_no_compare();
    let mut order = BreakOrder::default();
    front.step(&advance(1, 1), &mut order);
    assert_eq!(front.step(&reply(0, 1), &mut order), Step::Written);
    assert_eq!(order.check(), Err(BREAK_ORDER));
}

#[test]
fn the_stream_is_ordered_and_bounded() {
    let mut stream = FrontStream::default();
    assert!(stream.push(advance(1, 1)));
    assert!(stream.push(reply(0, 1)));
    assert_eq!(stream.pop(), Some(advance(1, 1)));
    assert_eq!(stream.pop(), Some(reply(0, 1)));
    assert_eq!(stream.pop(), None);
    for _ in 0..FRONT_CAP {
        assert!(stream.push(reply(0, 1)));
    }
    assert!(!stream.push(reply(0, 1)));
    assert_eq!(stream.len(), FRONT_CAP);
}
