// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests of the checks of a route socket's receive (ADR 0005, sheet 23):
//! the sender check of point 6.5 (short addresses, a wrong family, a port
//! other than the kernel's), truncation, and the errors.

use std::io;

use super::{Received, RouteError, Rules, checked, receive_error, sent};

const RULES: Rules = Rules {
    family: 16,
    name_length: 12,
    truncated: 32,
    overflow: 105,
};

/// A whole 100-byte datagram from the kernel, into a 4096-byte buffer.
const KERNEL: Received = Received {
    count: 100,
    capacity: 4096,
    flags: 0,
    name_length: 12,
    family: 16,
    port: 0,
};

#[test]
fn a_whole_datagram_from_the_kernel_is_returned_with_its_length() {
    assert_eq!(checked(RULES, &KERNEL).unwrap(), 100);
    let empty = Received { count: 0, ..KERNEL };
    assert_eq!(checked(RULES, &empty).unwrap(), 0);
    let full = Received {
        count: 4096,
        ..KERNEL
    };
    assert_eq!(checked(RULES, &full).unwrap(), 4096);
}

#[test]
fn an_address_of_another_length_or_family_is_no_sender() {
    for length in [0, 2, 4, 8, 11, 13, 16, 28, 128, usize::MAX] {
        let short = Received {
            name_length: length,
            ..KERNEL
        };
        assert!(
            matches!(checked(RULES, &short), Err(RouteError::BadSender)),
            "{length}"
        );
    }
    for family in [0, 1, 2, 10, 17, u16::MAX] {
        let wrong = Received { family, ..KERNEL };
        assert!(
            matches!(checked(RULES, &wrong), Err(RouteError::BadSender)),
            "{family}"
        );
    }
}

#[test]
fn a_port_other_than_the_kernel_s_is_refused_after_the_address_checks() {
    for port in [1, 4242, u32::MAX] {
        let other = Received { port, ..KERNEL };
        assert!(
            matches!(checked(RULES, &other), Err(RouteError::NotKernel(found)) if found == port),
            "{port}"
        );
        // A port is read only from an address that passed the other two
        // checks.
        let short = Received {
            port,
            name_length: 8,
            ..KERNEL
        };
        assert!(matches!(checked(RULES, &short), Err(RouteError::BadSender)));
        let wrong = Received {
            port,
            family: 2,
            ..KERNEL
        };
        assert!(matches!(checked(RULES, &wrong), Err(RouteError::BadSender)));
    }
}

#[test]
fn a_truncated_datagram_is_not_returned() {
    let truncated = Received {
        flags: 32,
        ..KERNEL
    };
    assert!(matches!(
        checked(RULES, &truncated),
        Err(RouteError::Truncated)
    ));
    let with_others = Received {
        flags: 32 | 64 | 0x4000_0000,
        ..KERNEL
    };
    assert!(matches!(
        checked(RULES, &with_others),
        Err(RouteError::Truncated)
    ));
    let longer = Received {
        count: 4097,
        ..KERNEL
    };
    assert!(matches!(
        checked(RULES, &longer),
        Err(RouteError::Truncated)
    ));
    // A forged sender is named as such even when its datagram is long.
    let forged = Received {
        flags: 32,
        port: 7,
        ..KERNEL
    };
    assert!(matches!(
        checked(RULES, &forged),
        Err(RouteError::NotKernel(7))
    ));
}

#[test]
fn the_sweep_of_every_field_returns_only_the_kernel_s_whole_datagram() {
    for name_length in [0, 11, 12, 13] {
        for family in [0, 16, 17] {
            for port in [0, 1] {
                for flags in [0, 32] {
                    for count in [0, 4096, 4097] {
                        let received = Received {
                            count,
                            capacity: 4096,
                            flags,
                            name_length,
                            family,
                            port,
                        };
                        let valid = name_length == 12
                            && family == 16
                            && port == 0
                            && flags == 0
                            && count <= 4096;
                        assert_eq!(checked(RULES, &received).is_ok(), valid, "{received:?}");
                    }
                }
            }
        }
    }
}

#[test]
fn a_receive_error_tells_an_overflow_apart() {
    assert!(matches!(
        receive_error(RULES, io::Error::from_raw_os_error(105)),
        RouteError::Overflow(error) if error.raw_os_error() == Some(105)
    ));
    assert!(matches!(
        receive_error(RULES, io::Error::from_raw_os_error(9)),
        RouteError::Receive(error) if error.raw_os_error() == Some(9)
    ));
}

#[test]
fn a_send_of_fewer_bytes_than_the_request_is_short() {
    sent(20, 20).unwrap();
    assert!(matches!(
        sent(19, 20),
        Err(RouteError::ShortSend {
            sent: 19,
            length: 20
        })
    ));
    assert!(matches!(
        sent(0, 20),
        Err(RouteError::ShortSend {
            sent: 0,
            length: 20
        })
    ));
    assert!(matches!(
        sent(21, 20),
        Err(RouteError::ShortSend {
            sent: 21,
            length: 20
        })
    ));
}
