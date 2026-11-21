// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The interleaving witness of a race scenario (seams and scheduling
//! specification, 5.6, point 6; test strategy specification, K5).
//!
//! A uniform draw among enabled entries often gives a schedule in which
//! the racing tasks never meet inside the window that matters, so a race
//! is swept over a range of seeds, and each seed's witness is the order of
//! its two named meeting events inside the critical window, both naming
//! the window's object. A seed where either is missing, or names another
//! object, is not exercised: never a pass. The race fails unless each
//! order it claims is witnessed on at least its floor of seeds. Task
//! hand-offs are not counted: only the named events are.

use std::fmt;

use crate::trace::{Op, ops};

#[cfg(test)]
mod tests;

/// A race: its critical window and its two meeting events, by the labels
/// of the operation events that mark them.
#[derive(Clone, Copy, Debug)]
pub struct Race {
    /// The event that opens the window; its object is the contested one.
    pub open: &'static str,
    /// The first event after it, on the same object, that closes it.
    pub close: &'static str,
    /// The first meeting event.
    pub first: &'static str,
    /// The second meeting event.
    pub second: &'static str,
    /// The fewest seeds each claimed order must be witnessed on.
    pub floor: usize,
}

/// The order of the two meeting events inside the window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Order {
    /// The first meeting event, then the second.
    FirstThenSecond,
    /// The second meeting event, then the first.
    SecondThenFirst,
}

impl Race {
    /// The witness of one trace: the order in which the two meeting
    /// events occurred inside the window, or `None` when the seed did not
    /// exercise the race.
    #[must_use]
    pub fn witness(&self, trace: &str) -> Option<Order> {
        let events = ops(trace);
        let open_at = events.iter().position(|op| op.label == self.open)?;
        let object = &events.get(open_at)?.object;
        let inside = events.get(open_at.saturating_add(1)..)?;
        let close_at = inside
            .iter()
            .position(|op| op.label == self.close && op.object == *object)?;
        let window = inside.get(..close_at)?;
        let find = |label: &str| -> Option<(usize, &Op)> {
            window.iter().enumerate().find(|(_, op)| op.label == label)
        };
        let (first_at, first) = find(self.first)?;
        let (second_at, second) = find(self.second)?;
        if first.object != *object || second.object != *object {
            return None;
        }
        Some(if first_at < second_at {
            Order::FirstThenSecond
        } else {
            Order::SecondThenFirst
        })
    }

    /// Sweeps the traces of a seed range and counts each witness.
    pub fn sweep<'a>(&self, traces: impl IntoIterator<Item = &'a str>) -> Report {
        let mut report = Report::default();
        for trace in traces {
            match self.witness(trace) {
                Some(Order::FirstThenSecond) => report.first_then_second += 1,
                Some(Order::SecondThenFirst) => report.second_then_first += 1,
                None => report.not_exercised += 1,
            }
        }
        report
    }
}

/// How many seeds witnessed each order, and how many exercised neither.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Report {
    /// Seeds whose first meeting event came first.
    pub first_then_second: usize,
    /// Seeds whose second meeting event came first.
    pub second_then_first: usize,
    /// Seeds that did not exercise the race.
    pub not_exercised: usize,
}

/// A claimed order witnessed on fewer seeds than the floor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Unwitnessed {
    /// The order.
    pub order: Order,
    /// The seeds that witnessed it.
    pub witnessed: usize,
    /// The floor.
    pub floor: usize,
    /// The report it came from.
    pub report: Report,
}

impl fmt::Display for Unwitnessed {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "order {:?} witnessed on {} seeds, below the floor of {} ({} exercised the other order, \
             {} not exercised)",
            self.order,
            self.witnessed,
            self.floor,
            match self.order {
                Order::FirstThenSecond => self.report.second_then_first,
                Order::SecondThenFirst => self.report.first_then_second,
            },
            self.report.not_exercised
        )
    }
}

impl std::error::Error for Unwitnessed {}

impl Report {
    /// Checks that each claimed order was witnessed on at least the
    /// race's floor of seeds.
    ///
    /// # Errors
    ///
    /// Returns the first claimed order below the floor.
    pub fn check(&self, race: &Race, claimed: &[Order]) -> Result<(), Unwitnessed> {
        for order in claimed {
            let witnessed = match order {
                Order::FirstThenSecond => self.first_then_second,
                Order::SecondThenFirst => self.second_then_first,
            };
            if witnessed < race.floor {
                return Err(Unwitnessed {
                    order: *order,
                    witnessed,
                    floor: race.floor,
                    report: *self,
                });
            }
        }
        Ok(())
    }
}
