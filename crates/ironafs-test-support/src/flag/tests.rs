// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for the hand-set flag: its state, and that it registers without
//! ever waking.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use ironafs_time::{Registration, Waker, Watch};

use super::Flag;

struct Counted(AtomicBool);

impl Waker for Counted {
    fn wake(&self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

#[test]
fn flag_default_is_not_set() {
    assert!(!Flag::default().is_set());
}

#[test]
fn flag_set_is_set() {
    assert!(Flag::set().is_set());
}

#[test]
fn flag_registers_and_never_wakes() {
    let waker = Arc::new(Counted(AtomicBool::new(false)));
    let flag = Flag::set();
    let registration = flag.register(waker.clone()).unwrap();
    assert_eq!(registration, Registration::new(0));
    flag.deregister(registration);
    assert!(!waker.0.load(Ordering::SeqCst));
}
