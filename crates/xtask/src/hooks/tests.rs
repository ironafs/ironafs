// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for recognition of a managed hook.

use super::{HOOK_HEADER, is_managed_hook};

#[test]
fn hook_recognizes_its_header_at_the_start() {
    let mut contents = HOOK_HEADER.as_bytes().to_vec();
    contents.extend_from_slice(b"exec cargo xtask check\n");

    assert!(is_managed_hook(&contents));
}

#[test]
fn hook_rejects_foreign_scripts_that_reference_xtask() {
    let mut contents = b"#!/bin/sh\nexec cargo xtask check\n".to_vec();
    assert!(!is_managed_hook(&contents));

    contents.extend_from_slice(HOOK_HEADER.as_bytes());
    assert!(!is_managed_hook(&contents));
}
