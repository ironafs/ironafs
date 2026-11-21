// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The abort codes the models return: the values of the specification's
//! abort table (`docs/specs/vl-fs-read.md`, 5.9), by its row.

/// R4: the server is restarting.
pub const SERVER_RESTARTING: i32 = -100;
/// R5: a marshalling failure; the models return it for a request the
/// envelope cannot decode.
pub const MARSHALLING: i32 = -451;
/// R6: the server does not know the procedure.
pub const UNKNOWN_PROCEDURE: i32 = -455;
/// R8: no such vnode.
pub const NO_VNODE: i32 = 102;
/// R9: the volume is not on this server, or not on line.
pub const NOT_ON_SERVER: i32 = 103;
/// R10: the volume is off line.
pub const VOLUME_OFFLINE: i32 = 106;
/// R11: the volume is busy for a while.
pub const VOLUME_BUSY: i32 = 110;
/// R12: the volume has moved.
pub const VOLUME_MOVED: i32 = 111;
/// R17: the volume location service has no such entry.
pub const VL_NO_ENTRY: i32 = 363_524;
