// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests of `unix-nodump`. Under Miri, the three calls are made in the
//! test process itself and reach their stand-ins, so the wrappers' local
//! `rlimit`, its pointer and the variadic arguments run unchanged, and
//! `stand_in` drives each failure through the wrapper. On the running
//! kernel, every test takes the controls in a child process (`kernel`):
//! they set both core-file limits to zero, which an unprivileged process
//! cannot undo and its children inherit, so a test process that took them
//! would leave a later child of the same harness no core to dump.

#[cfg(not(miri))]
mod kernel;
#[cfg(miri)]
mod stand_in;

#[cfg(miri)]
use super::{disable_dumps, dumpable};

#[test]
#[cfg(miri)]
fn disable_dumps_in_this_process_reads_back_not_dumpable() {
    disable_dumps().unwrap();
    assert_eq!(dumpable().unwrap(), 0);
    // Taking them again is the reapply rule after a transition.
    disable_dumps().unwrap();
    assert_eq!(dumpable().unwrap(), 0);
}
