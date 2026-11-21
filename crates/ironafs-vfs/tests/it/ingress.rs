// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! What an ingress record's debug output shows.

use std::sync::Arc;

use ironafs_test_support::Flag;
use ironafs_time::{AwakeInstant, BootInstant, Doubt, Stamp, WallTime};
use ironafs_vfs::{Ingress, RequestId};

fn ingress(cancel: Flag) -> Ingress {
    Ingress {
        request: RequestId(42),
        arrival: Stamp {
            boot: BootInstant::from_nanos(5),
            awake: AwakeInstant::from_nanos(4),
            wall: WallTime::from_unix(1, 0).unwrap(),
            doubt: Doubt { boot: 0, awake: 0 },
            episodes: 0,
        },
        cancel: Arc::new(cancel),
    }
}

#[test]
fn ingress_debug_shows_the_request_arrival_and_whether_it_was_cancelled() {
    let shown = format!("{:?}", ingress(Flag::set()));
    assert!(shown.contains("RequestId(42)"), "{shown}");
    assert!(shown.contains("nanos: 5"), "{shown}");
    assert!(shown.contains("cancelled: true"), "{shown}");
    let shown = format!("{:?}", ingress(Flag::default()));
    assert!(shown.contains("cancelled: false"), "{shown}");
}
