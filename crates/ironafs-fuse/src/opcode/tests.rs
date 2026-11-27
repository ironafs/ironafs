// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for the opcode table (fuse-codec.md 5.1, point 4; 5.4, point 9;
//! 5.6, point 1).

use super::Opcode;

/// E70's opcode table, written out by hand.
const TABLE: &[(u32, Opcode)] = &[
    (1, Opcode::Lookup),
    (2, Opcode::Forget),
    (3, Opcode::Getattr),
    (4, Opcode::Setattr),
    (5, Opcode::Readlink),
    (6, Opcode::Symlink),
    (8, Opcode::Mknod),
    (9, Opcode::Mkdir),
    (10, Opcode::Unlink),
    (11, Opcode::Rmdir),
    (12, Opcode::Rename),
    (13, Opcode::Link),
    (14, Opcode::Open),
    (15, Opcode::Read),
    (16, Opcode::Write),
    (17, Opcode::Statfs),
    (18, Opcode::Release),
    (20, Opcode::Fsync),
    (21, Opcode::Setxattr),
    (22, Opcode::Getxattr),
    (23, Opcode::Listxattr),
    (24, Opcode::Removexattr),
    (25, Opcode::Flush),
    (26, Opcode::Init),
    (27, Opcode::Opendir),
    (28, Opcode::Readdir),
    (29, Opcode::Releasedir),
    (30, Opcode::Fsyncdir),
    (31, Opcode::Getlk),
    (32, Opcode::Setlk),
    (33, Opcode::Setlkw),
    (34, Opcode::Access),
    (35, Opcode::Create),
    (36, Opcode::Interrupt),
    (37, Opcode::Bmap),
    (38, Opcode::Destroy),
    (39, Opcode::Ioctl),
    (40, Opcode::Poll),
    (41, Opcode::NotifyReply),
    (42, Opcode::BatchForget),
    (43, Opcode::Fallocate),
    (44, Opcode::Readdirplus),
    (45, Opcode::Rename2),
    (46, Opcode::Lseek),
    (47, Opcode::CopyFileRange),
    (48, Opcode::Setupmapping),
    (49, Opcode::Removemapping),
    (50, Opcode::Syncfs),
    (51, Opcode::Tmpfile),
    (52, Opcode::Statx),
    (53, Opcode::CopyFileRange64),
];

#[test]
fn from_u32_knows_every_documented_opcode() {
    for &(number, opcode) in TABLE {
        assert_eq!(Opcode::from_u32(number), Some(opcode), "{number}");
        assert_eq!(opcode.number(), number, "{opcode:?}");
    }
}

#[test]
fn unknown_numbers_are_none() {
    // 7 and 19 are gaps in E70's table; 4096 is CUSE_INIT and the last two
    // the byte-swapped INIT opcodes, none of them a FUSE request.
    for number in [0, 7, 19, 54, u32::MAX, 4096, 1_048_576, 436_207_616] {
        assert_eq!(Opcode::from_u32(number), None, "{number}");
    }
}

#[test]
fn no_reply_for_forgets_and_interrupt() {
    for &(_, opcode) in TABLE {
        let silent = matches!(
            opcode,
            Opcode::Forget | Opcode::BatchForget | Opcode::Interrupt
        );
        assert_eq!(opcode.needs_reply(), !silent, "{opcode:?}");
    }
}
