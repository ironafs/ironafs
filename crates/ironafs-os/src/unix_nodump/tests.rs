// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests of `unix-nodump` that Miri runs too: the three calls in the
//! process itself, which under Miri reach their stand-ins, so the
//! wrappers' local `rlimit`, its pointer and the variadic arguments run
//! unchanged. Taking the controls in the test process is harmless: it only
//! makes that process non-dumpable. The tests that need the running kernel
//! (child processes, ids, core collectors) are in `kernel`.

use super::{disable_dumps, dumpable};

#[cfg(not(miri))]
mod kernel;

#[test]
fn disable_dumps_in_this_process_reads_back_not_dumpable() {
    disable_dumps().unwrap();
    assert_eq!(dumpable().unwrap(), 0);
    // Taking them again is the reapply rule after a transition.
    disable_dumps().unwrap();
    assert_eq!(dumpable().unwrap(), 0);
}
