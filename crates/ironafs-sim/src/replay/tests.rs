// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The replay log reads picks and compares lines in order.

use super::Replay;
use crate::sched::Failure;

const TRACE: &str = "ironafs-sim trace 3\n\
                     d0 sched decide [t0,t1] pick=1 drawn draws=1 at=0\n\
                     d0 t1 op a x\n\
                     d1 sched decide [t0] pick=0 drawn draws=2 at=10\n";

#[test]
fn picks_come_from_the_log_and_a_missing_one_diverges() {
    let replay = Replay::from_trace(TRACE);
    assert_eq!(replay.pick(0), Ok(1));
    assert_eq!(replay.pick(1), Ok(0));
    assert_eq!(replay.pick(2), Err(Failure::Diverged { decision: 2 }));
}

#[test]
fn lines_are_compared_in_order_until_exhausted() {
    let mut replay = Replay::from_trace(TRACE);
    let lines: Vec<&str> = TRACE.lines().collect();
    assert!(replay.matches_next(lines[0]));
    assert!(!replay.matches_next("d0 sched decide [t0,t1] pick=0 drawn draws=1 at=0"));
    assert!(replay.matches_next(lines[2]));
    assert!(!replay.is_exhausted());
    assert!(replay.matches_next(lines[3]));
    assert!(replay.is_exhausted());
    assert!(!replay.matches_next("d2 sched end ok"));
}
