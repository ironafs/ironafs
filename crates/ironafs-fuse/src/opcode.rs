// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The FUSE opcodes of the kernel's interface definition, protocol 7.46
//! (fuse-codec.md 5.1, point 4, and 5.4, point 9), and which of them take
//! no reply (5.6, point 1).

#[cfg(test)]
mod tests;

/// Whether a request with this opcode number is given a reply token:
/// every request except FORGET, `BATCH_FORGET` and INTERRUPT, an opcode
/// the interface does not define included. The decoder and the session
/// both ask this one function, so they cannot disagree.
pub(crate) fn takes_reply(number: u32) -> bool {
    Opcode::from_u32(number).is_none_or(Opcode::needs_reply)
}

/// A FUSE request's operation. Each variant's value is its number on the
/// wire. `CUSE_INIT` and the byte-swapped INIT opcodes are not FUSE
/// requests and have no variant.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u32)]
#[expect(
    missing_docs,
    reason = "each variant is the kernel opcode of the same name"
)]
pub enum Opcode {
    Lookup = 1,
    Forget = 2,
    Getattr = 3,
    Setattr = 4,
    Readlink = 5,
    Symlink = 6,
    Mknod = 8,
    Mkdir = 9,
    Unlink = 10,
    Rmdir = 11,
    Rename = 12,
    Link = 13,
    Open = 14,
    Read = 15,
    Write = 16,
    Statfs = 17,
    Release = 18,
    Fsync = 20,
    Setxattr = 21,
    Getxattr = 22,
    Listxattr = 23,
    Removexattr = 24,
    Flush = 25,
    Init = 26,
    Opendir = 27,
    Readdir = 28,
    Releasedir = 29,
    Fsyncdir = 30,
    Getlk = 31,
    Setlk = 32,
    Setlkw = 33,
    Access = 34,
    Create = 35,
    Interrupt = 36,
    Bmap = 37,
    Destroy = 38,
    Ioctl = 39,
    Poll = 40,
    NotifyReply = 41,
    BatchForget = 42,
    Fallocate = 43,
    Readdirplus = 44,
    Rename2 = 45,
    Lseek = 46,
    CopyFileRange = 47,
    Setupmapping = 48,
    Removemapping = 49,
    Syncfs = 50,
    Tmpfile = 51,
    Statx = 52,
    CopyFileRange64 = 53,
}

impl Opcode {
    /// The opcode with this number, or `None` for a number the interface
    /// does not define.
    #[must_use]
    pub const fn from_u32(number: u32) -> Option<Self> {
        Some(match number {
            1 => Self::Lookup,
            2 => Self::Forget,
            3 => Self::Getattr,
            4 => Self::Setattr,
            5 => Self::Readlink,
            6 => Self::Symlink,
            8 => Self::Mknod,
            9 => Self::Mkdir,
            10 => Self::Unlink,
            11 => Self::Rmdir,
            12 => Self::Rename,
            13 => Self::Link,
            14 => Self::Open,
            15 => Self::Read,
            16 => Self::Write,
            17 => Self::Statfs,
            18 => Self::Release,
            20 => Self::Fsync,
            21 => Self::Setxattr,
            22 => Self::Getxattr,
            23 => Self::Listxattr,
            24 => Self::Removexattr,
            25 => Self::Flush,
            26 => Self::Init,
            27 => Self::Opendir,
            28 => Self::Readdir,
            29 => Self::Releasedir,
            30 => Self::Fsyncdir,
            31 => Self::Getlk,
            32 => Self::Setlk,
            33 => Self::Setlkw,
            34 => Self::Access,
            35 => Self::Create,
            36 => Self::Interrupt,
            37 => Self::Bmap,
            38 => Self::Destroy,
            39 => Self::Ioctl,
            40 => Self::Poll,
            41 => Self::NotifyReply,
            42 => Self::BatchForget,
            43 => Self::Fallocate,
            44 => Self::Readdirplus,
            45 => Self::Rename2,
            46 => Self::Lseek,
            47 => Self::CopyFileRange,
            48 => Self::Setupmapping,
            49 => Self::Removemapping,
            50 => Self::Syncfs,
            51 => Self::Tmpfile,
            52 => Self::Statx,
            53 => Self::CopyFileRange64,
            _ => return None,
        })
    }

    /// The opcode's number on the wire.
    #[must_use]
    pub const fn number(self) -> u32 {
        self as u32
    }

    /// Whether the kernel waits for a reply: every request except FORGET,
    /// `BATCH_FORGET` and INTERRUPT (fuse-codec.md 5.6, point 1).
    #[must_use]
    pub const fn needs_reply(self) -> bool {
        !matches!(self, Self::Forget | Self::BatchForget | Self::Interrupt)
    }
}
