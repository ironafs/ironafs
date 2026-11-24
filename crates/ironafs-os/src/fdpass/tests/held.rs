// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! A stand-in descriptor for the receive checks' tests and their fuzz
//! target, which includes this file by its path: an adopter that records
//! each integer it adopts, and a [`Held`] that records its drop, so a run
//! shows which integers were adopted and that none outlives a refusal. It
//! never makes an integer a descriptor.

use std::cell::RefCell;
use std::rc::Rc;

/// What happened to the stand-in descriptors: adopted, and dropped.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Log {
    pub(crate) adopted: Vec<i32>,
    pub(crate) dropped: Vec<i32>,
}

/// A stand-in descriptor: records its drop.
pub(crate) struct Held(pub(crate) i32, Rc<RefCell<Log>>);

impl Drop for Held {
    fn drop(&mut self) {
        self.1.borrow_mut().dropped.push(self.0);
    }
}

/// The adopter for `accept`: records each integer in `log` and hands back
/// a [`Held`] for it.
pub(crate) fn adopter(log: &Rc<RefCell<Log>>) -> impl FnMut(i32) -> Held + '_ {
    move |value| {
        log.borrow_mut().adopted.push(value);
        Held(value, Rc::clone(log))
    }
}
