// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The `Rpc` error's text is a fixed vocabulary (seams and scheduling
//! specification, 5.6, point 11 (c)), and says which side of a send it is.

use alloc::string::ToString;

use super::{
    AddrFamily, EndpointGeneration, NoReplyCause, NotSentCause, ReplyEvidence, RpcError,
    SecurityFailure,
};

#[test]
fn rpc_error_messages_name_the_side_of_the_send() {
    assert_eq!(
        RpcError::NotSent(NotSentCause::Admission).to_string(),
        "not sent: admission"
    );
    assert_eq!(
        RpcError::NoReply(NoReplyCause::Security(SecurityFailure::new())).to_string(),
        "no reply: security"
    );
    let via = ReplyEvidence {
        family: AddrFamily::V6,
        local_generation: EndpointGeneration(1),
        epoch: 2,
        conn_id: 3,
        call_number: 4,
        received: None,
    };
    assert_eq!(
        RpcError::Aborted { code: 5, via }.to_string(),
        "aborted with code 5"
    );
}
