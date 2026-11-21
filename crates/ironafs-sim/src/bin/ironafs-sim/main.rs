// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The simulator's child process: runs or replays one scenario (see
//! `ironafs_sim::child`).

use std::io;
use std::process::ExitCode;

fn main() -> ExitCode {
    let exit = ironafs_sim::child::main(
        std::env::args().skip(1),
        &mut io::stdin().lock(),
        Box::new(io::stdout()),
        &mut io::stderr().lock(),
    );
    ExitCode::from(exit as u8)
}
