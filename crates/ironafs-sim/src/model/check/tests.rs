// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The continuity and break-order checks fail exactly on the orders ADR
//! 0004, O45, point 3, forbids.

use super::{BREAK_ORDER, Binding, BreakOrder, CONTINUITY, Continuity};
use crate::model::types::{Addr, Fid};

const FS1: Addr = Addr([10, 0, 0, 2]);
const FS2: Addr = Addr([10, 0, 0, 3]);

fn fid() -> Fid {
    Fid::new(100, 2, 1)
}

#[test]
fn a_stale_read_under_trust_passes_until_the_next_completed_call() {
    let mut check = Continuity::default();
    check.read(FS1, 1, 2, true);
    check.break_failed(FS1);
    check.read(FS1, 1, 2, true);
    assert_eq!(check.check(), Ok(()));
    check.call_completed(FS2);
    check.read(FS1, 1, 2, true);
    assert_eq!(
        check.check(),
        Ok(()),
        "a call to another server changes nothing"
    );
    check.call_completed(FS1);
    check.read(FS1, 2, 2, true);
    check.read(FS1, 1, 2, false);
    assert_eq!(check.check(), Ok(()), "fresh, or stale without trust");
    check.read(FS1, 1, 2, true);
    assert_eq!(check.check(), Err(CONTINUITY));
}

#[test]
fn a_completed_call_before_any_failure_allows_nothing_new() {
    let mut check = Continuity::default();
    check.call_completed(FS1);
    check.break_failed(FS1);
    check.read(FS1, 1, 2, true);
    assert_eq!(check.check(), Ok(()));
    check.call_completed(FS1);
    check.read(FS1, 1, 3, true);
    assert_eq!(check.check(), Err(CONTINUITY));
}

#[test]
fn a_reply_submitted_after_its_break_is_effective_breaks_the_order() {
    for binding in [Binding::InProcess, Binding::FrontEnd] {
        let mut order = BreakOrder::default();
        order.submitted(binding, fid(), 1);
        order.effective(binding, fid(), 1);
        order.submitted(binding, fid(), 2);
        assert_eq!(order.check(), Ok(()), "{binding:?}");
        order.submitted(binding, fid(), 1);
        assert_eq!(order.check(), Err(BREAK_ORDER), "{binding:?}");
    }
}

#[test]
fn a_break_effective_at_one_binding_does_not_bind_the_other() {
    let mut order = BreakOrder::default();
    order.effective(Binding::InProcess, fid(), 1);
    order.submitted(Binding::FrontEnd, fid(), 1);
    order.submitted(Binding::InProcess, Fid::new(100, 4, 1), 1);
    assert_eq!(order.check(), Ok(()));
}
