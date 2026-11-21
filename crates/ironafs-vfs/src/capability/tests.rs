// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for minting capabilities and for what their debug output shows.

use alloc::format;

use super::{BindingId, ContextId, CredentialGeneration, HandleId, Mint};

const NONCE: u128 = 0x0123_4567_89ab_cdef_fedc_ba98_7654_3210;

#[test]
fn caller_capability_carries_what_was_minted() {
    let cap = Mint::new().caller(BindingId(7), ContextId(11), CredentialGeneration(3), NONCE);
    assert_eq!(cap.binding(), BindingId(7));
    assert_eq!(cap.context(), ContextId(11));
    assert_eq!(cap.generation(), CredentialGeneration(3));
    assert_eq!(cap.nonce(), NONCE);
}

#[test]
fn handle_capability_carries_what_was_minted() {
    let cap = Mint::new().handle(BindingId(7), HandleId(19), NONCE);
    assert_eq!(cap.binding(), BindingId(7));
    assert_eq!(cap.handle(), HandleId(19));
    assert_eq!(cap.nonce(), NONCE);
}

#[test]
fn capability_debug_hides_the_nonce() {
    let caller = Mint::new().caller(BindingId(7), ContextId(11), CredentialGeneration(3), NONCE);
    let handle = Mint::new().handle(BindingId(7), HandleId(19), NONCE);
    for shown in [format!("{caller:?}"), format!("{handle:?}")] {
        assert!(!shown.contains("nonce"), "{shown}");
        assert!(!shown.contains(&format!("{NONCE}")), "{shown}");
        assert!(!shown.contains(&format!("{NONCE:x}")), "{shown}");
        assert!(shown.contains("BindingId(7)"), "{shown}");
    }
}
