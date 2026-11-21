// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Catching panics in property code without printing each one.
//!
//! Shrinking runs a failing property many times, and the default panic
//! hook would print every panic. A wrapper around the process's panic hook,
//! installed once, stays silent on a thread while that thread is inside
//! [`catch_quietly`] and defers to the previous hook otherwise, so panics
//! elsewhere, including other tests running in parallel, print as before.
//! A hook installed later by other code replaces the wrapper; panics are
//! then printed again, which is noise and not a wrong result.

use std::any::Any;
use std::cell::Cell;
use std::panic::{self, AssertUnwindSafe};
use std::sync::Once;

#[cfg(test)]
mod tests;

thread_local! {
    static QUIET: Cell<bool> = const { Cell::new(false) };
}

static INSTALL_HOOK: Once = Once::new();

/// Runs `run`, returning its value, or the message of the panic it raised.
///
/// The panic is not printed. State `run` shares with the caller may be
/// left half updated by the panic; property code is run on its own inputs,
/// so the runner does not observe that state again.
pub(crate) fn catch_quietly<R>(run: impl FnOnce() -> R) -> Result<R, String> {
    INSTALL_HOOK.call_once(|| {
        let previous = panic::take_hook();
        panic::set_hook(Box::new(move |info| {
            if !QUIET.with(Cell::get) {
                previous(info);
            }
        }));
    });
    let was_quiet = QUIET.with(|quiet| quiet.replace(true));
    let result = panic::catch_unwind(AssertUnwindSafe(run));
    QUIET.with(|quiet| quiet.set(was_quiet));
    result.map_err(|payload| panic_message(payload.as_ref()))
}

/// The text of a panic payload: `panic!` and `assert!` give a `&str` or a
/// `String`.
fn panic_message(payload: &(dyn Any + Send)) -> String {
    if let Some(text) = payload.downcast_ref::<&str>() {
        (*text).to_owned()
    } else if let Some(text) = payload.downcast_ref::<String>() {
        text.clone()
    } else {
        "a panic whose payload is not a string".to_owned()
    }
}
