// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

use alloc::vec::Vec;

use super::{Ledger, Reservation};
use crate::ErrorKind;

#[test]
fn reserve_records_count_times_element_size() {
    let mut seen = Vec::new();
    let mut hook = |reservation| seen.push(reservation);
    {
        let mut ledger = Ledger::new("test.m", 64, Some(&mut hook));
        let buffer = ledger.reserve::<u32>(3, "test.m.f").unwrap();
        assert!(buffer.capacity() >= 3);
    }
    assert_eq!(
        seen,
        [Reservation {
            bytes: 12,
            name: "test.m.f"
        }]
    );
}

#[test]
fn reserve_refuses_past_budget_and_records_nothing() {
    let mut seen = Vec::new();
    let mut hook = |reservation| seen.push(reservation);
    {
        let mut ledger = Ledger::new("test.m", 10, Some(&mut hook));
        ledger.reserve::<u8>(6, "test.m.a").unwrap();
        let error = ledger.reserve::<u8>(5, "test.m.b").unwrap_err();
        assert_eq!(error.kind(), ErrorKind::BudgetExceeded);
        assert_eq!(error.name(), Some("test.m"));
        assert_eq!(error.limit_value(), Some(10));
    }
    assert_eq!(
        seen,
        [Reservation {
            bytes: 6,
            name: "test.m.a"
        }]
    );
}

#[test]
fn reserve_up_to_exactly_the_budget_passes() {
    let mut ledger = Ledger::new("test.m", 8, None);
    ledger.reserve::<u8>(8, "test.m.a").unwrap();
    assert_eq!(
        ledger.reserve::<u8>(1, "test.m.a").unwrap_err().kind(),
        ErrorKind::BudgetExceeded
    );
}

#[test]
fn reserve_of_zero_sized_type_records_zero_bytes() {
    let mut seen = Vec::new();
    let mut hook = |reservation| seen.push(reservation);
    {
        let mut ledger = Ledger::new("test.m", 0, Some(&mut hook));
        ledger.reserve::<()>(1000, "test.m.z").unwrap();
    }
    assert_eq!(
        seen,
        [Reservation {
            bytes: 0,
            name: "test.m.z"
        }]
    );
}

#[test]
fn reserve_whose_size_overflows_is_over_budget() {
    let mut ledger = Ledger::new("test.m", u64::MAX, None);
    assert_eq!(
        ledger
            .reserve::<u64>(usize::MAX, "test.m.a")
            .unwrap_err()
            .kind(),
        ErrorKind::BudgetExceeded
    );
}
