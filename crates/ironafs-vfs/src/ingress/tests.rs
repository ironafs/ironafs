// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for what an ingress record's debug output shows.

use alloc::format;
use alloc::sync::Arc;
use core::sync::atomic::{AtomicBool, Ordering};

use ironafs_time::{
    AwakeInstant, BootInstant, Doubt, Registration, Stamp, Waker, WallTime, Watch, WatchFull,
};

use super::{Ingress, RequestId};

/// A cancel flag that is set or not, and never wakes anyone.
struct Flag(AtomicBool);

impl Watch for Flag {
    fn is_set(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }

    fn register(&self, _waker: Arc<dyn Waker>) -> Result<Registration, WatchFull> {
        Ok(Registration::new(0))
    }

    fn deregister(&self, _registration: Registration) {}
}

fn stamp() -> Stamp {
    Stamp {
        boot: BootInstant::from_nanos(5),
        awake: AwakeInstant::from_nanos(4),
        wall: WallTime::from_unix(1, 0).unwrap(),
        doubt: Doubt { boot: 0, awake: 0 },
        episodes: 0,
    }
}

#[test]
fn ingress_debug_shows_the_request_and_whether_it_was_cancelled() {
    let ingress = Ingress {
        request: RequestId(42),
        arrival: stamp(),
        cancel: Arc::new(Flag(AtomicBool::new(true))),
    };
    let shown = format!("{ingress:?}");
    assert!(shown.contains("RequestId(42)"), "{shown}");
    assert!(shown.contains("cancelled: true"), "{shown}");
    assert!(shown.contains("nanos: 5"), "{shown}");
}
