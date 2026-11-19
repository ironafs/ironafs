// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Process entry point for `cargo xtask`.

use std::io;
use std::process;

fn main() {
    let status = xtask::run(
        std::env::args_os().skip(1),
        &mut io::stdout(),
        &mut io::stderr(),
    );
    if status != 0 {
        process::exit(status);
    }
}
