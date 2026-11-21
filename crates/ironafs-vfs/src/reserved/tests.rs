// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for the refusal of reserved operations.

use super::Reserved;
use crate::VfsError;

#[test]
fn every_reserved_operation_is_refused_as_not_supported() {
    for reserved in [
        Reserved::CopyRange { clone: false },
        Reserved::CopyRange { clone: true },
        Reserved::Allocate,
        Reserved::Punch,
        Reserved::ExtendedAttributeWrite,
        Reserved::NamedStream,
        Reserved::RenameFlags { no_replace: false },
        Reserved::RenameFlags { no_replace: true },
    ] {
        assert_eq!(reserved.refusal(), VfsError::NotSupported, "{reserved:?}");
    }
}
