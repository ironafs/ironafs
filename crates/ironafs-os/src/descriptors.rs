// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Test support for the kernel tests on Linux that check no descriptor is
//! leaked: the count of this process's open descriptors, read from
//! `/proc/self/fd`. The count includes the directory descriptor the read
//! itself holds, the same on every call, so two counts compare.

/// The number of this process's open descriptors.
pub(crate) fn open_descriptors() -> usize {
    std::fs::read_dir("/proc/self/fd").unwrap().count()
}
